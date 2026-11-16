/*
The core vocabulary catalog lists bundled core declarations that checking must
recognize by identity: the services, tokens, domains and private primitives
that typed rules are written against. Recognition is all a row grants. A
vocabulary row never gives compiler custody; that comes only from
`sealed_intrinsics`, and the two tables share no variant, so neither can be
read as the other.

Rows match exactly like sealed intrinsic rows: the package-relative file, the
declaration path and the exact bundled source bytes must all agree, and the
caller first establishes toolchain origin through `symbols::core_vocabulary_at`
or `SymbolTable::core_vocabulary`. A lookalike user declaration, or an edited
toolchain file, is therefore ordinary code to every consumer.
*/

use crate::sealed_intrinsics::{SealedDeclaration, SealedDeclarationSite};
use std::path::Path;

/// A core declaration that typed rules recognize by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreVocabulary {
    /// The ordinary `Never` data declaration in core `never.omg`.
    /// This identifies a nominal ABI result; it proves no inhabitedness.
    Never,
    /// The `MachineControl` service in core `assembly.omg`.
    MachineControlService,
    /// The `PortIo` service in core `assembly.omg`.
    PortIoService,
    /// A private segment-selector primitive in core `code_segment.omg` or
    /// `data_segment.omg`.
    SegmentSelector(SegmentSelectorOperation),
    /// A private task-register primitive in core `task_register.omg`.
    TaskRegister(TaskRegisterOperation),
    /// Core `interrupt.omg`'s linear `InterruptAcknowledgement` token.
    InterruptAcknowledgement,
    /// Core `interrupt.omg`'s `InterruptAcknowledgement::Pending` domain.
    InterruptAcknowledgementPending,
    /// Core `interrupt.omg`'s `InterruptAcknowledgementCompletion` trait.
    InterruptAcknowledgementCompletion,
    /// Its `complete` requirement.
    InterruptAcknowledgementCompletionComplete,
    /// Core `static_interrupt.omg`'s `StaticInterruptCompletionPermit` token.
    StaticInterruptCompletionPermit,
    /// A reflection operation or result type in core `reflect.omg`.
    Reflection(ReflectionDeclaration),
    /// The semantic access policy requirement and its result vocabulary.
    AccessPolicy,
    AccessPolicyPlan,
    AccessPlan,
    ResourceProfile,
    ResourceProfileSupply,
    ResourceProfileSupplyProfile,
    /// Core `extent.omg`'s linear `Extent` storage data.
    Extent,
    /// One of `Extent`'s declared domains in `extent.omg`.
    ExtentDomain(ExtentDomain),
    /// The `ProgramStorageEntry` arrival trait in `extent.omg`.
    ProgramStorageEntry,
    /// Typed-run nominal data and its live-cell predicate; recognition grants no custody.
    Run(RunDeclaration),
    /// Closed proof-only resource-content vocabulary; recognition grants no authority.
    Content(ContentDeclaration),
    /// Ordinary proof-natural data; recognizing it does not discharge conversion.
    ProofNatural,
    /// Core `fixed_vec.omg`'s `FixedVec<T, N>` data.
    FixedVec,
    /// A primitive instruction-effect proposition in core `assembly_model.omg`.
    AssemblyEffect(AssemblyEffect),
    /// A native-binding vocabulary type in core `external_binding.omg`.
    ExternalBinding(ExternalBindingVocabulary),
    /// Core `integer.omg`'s carrier metadata query or its result type.
    IntegerCarrier(IntegerCarrierDeclaration),
}

/// The closed algebra carriers and projection requirement from core `content.omg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentDeclaration {
    Content,
    Project,
    IntervalSet,
    CountedQuantity,
}

/// The exact declarations used by the compiler-supplied run custody family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunDeclaration {
    Run,
    Parts,
    Live,
    Singleton,
}

/// The `integer.omg` declarations the carrier-family classification is read
/// through (`wiki/spec/language/numeric_values.md`, "Integer carrier
/// families"). The four family queries are ordinary machines over `Query`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerCarrierDeclaration {
    /// `carrier<T>()`, answered from the type argument's carrier identity.
    Query,
    /// `Carrier`, the record `carrier<T>()` describes a type with.
    Carrier,
}

/// The primitive propositions `assembly_model.omg` declares for instruction effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssemblyEffect {
    PortWritten,
    ModelSpecificWritten,
    AtomicReleaseWritten,
}

/// The native-binding vocabulary types in `external_binding.omg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalBindingVocabulary {
    ForeignBinding,
    DllImport,
}

/// The domains `extent.omg` declares over `Extent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtentDomain {
    Vacant,
    Resident,
    Granted,
}

impl ExtentDomain {
    /// The domain's declared member name, as checking spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vacant => "Vacant",
            Self::Resident => "Resident",
            Self::Granted => "Granted",
        }
    }
}

/// The `reflect.omg` declarations reflection elaboration is written against.
/// They live in module `reflect`, so their package paths carry that prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReflectionDeclaration {
    /// `schema<T>()`.
    Schema,
    /// `callable(declaration)`, an uninvoked declaration selector.
    Callable,
    /// `field(declaration)`, an uninvoked field selector.
    Field,
    /// `parameter(declaration)`, an uninvoked boundary-formal selector.
    Parameter,
    /// `CallableInfo`, the descriptive result of `callable`.
    CallableInfo,
    /// `TypeSchema`, the result of `schema`.
    TypeSchema,
    /// `CaseInfo`, active-case metadata.
    CaseInfo,
    /// `FieldInfo`, field metadata.
    FieldInfo,
    ParameterInfo,
    VisitRuntimeFields,
    VisitRuntimeFieldsMut,
    InspectRecordFields,
    VisitActiveFields,
    VisitActiveFieldsMut,
}

impl CoreVocabulary {
    /// Whether this is the `extent.omg` domain declared with member `name`.
    /// Names `extent.omg` does not declare never match.
    pub fn is_extent_domain_named(self, name: &str) -> bool {
        matches!(self, Self::ExtentDomain(domain) if domain.name() == name)
    }
}

/// The selector primitives of the two segment files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SegmentSelectorOperation {
    ReloadCode,
    ReadCode,
    ReloadData,
    ReadData,
    ReadExtra,
    ReadStack,
    ReadFs,
    ReadGs,
}

/// The task-register primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskRegisterOperation {
    Activate,
    Read,
}

struct CoreVocabularyRow {
    file: &'static str,
    declaration: &'static str,
    source: &'static [u8],
    vocabulary: CoreVocabulary,
}

macro_rules! core_source {
    ($file:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/core/",
            $file
        ))
    };
}

const ASSEMBLY: &[u8] = core_source!("assembly.omg");
const CODE_SEGMENT: &[u8] = core_source!("code_segment.omg");
const DATA_SEGMENT: &[u8] = core_source!("data_segment.omg");
const TASK_REGISTER: &[u8] = core_source!("task_register.omg");
const INTERRUPT: &[u8] = core_source!("interrupt.omg");
const STATIC_INTERRUPT: &[u8] = crate::static_interrupts::CORE_DECLARATIONS;
const REFLECT: &[u8] = core_source!("reflect.omg");
const LAYOUT: &[u8] = core_source!("layout.omg");
const EXTENT: &[u8] = core_source!("extent.omg");
const FIXED_VEC: &[u8] = core_source!("fixed_vec.omg");
const RUN: &[u8] = core_source!("run.omg");
const CONTENT: &[u8] = core_source!("content.omg");
const NAT: &[u8] = core_source!("nat.omg");
const ASSEMBLY_MODEL: &[u8] = core_source!("assembly_model.omg");
const EXTERNAL_BINDING: &[u8] = core_source!("external_binding.omg");
const INTEGER: &[u8] = core_source!("integer.omg");
const NEVER: &[u8] = core_source!("never.omg");

const fn row(
    file: &'static str,
    declaration: &'static str,
    source: &'static [u8],
    vocabulary: CoreVocabulary,
) -> CoreVocabularyRow {
    CoreVocabularyRow {
        file,
        declaration,
        source,
        vocabulary,
    }
}

const fn selector(
    file: &'static str,
    declaration: &'static str,
    source: &'static [u8],
    operation: SegmentSelectorOperation,
) -> CoreVocabularyRow {
    row(
        file,
        declaration,
        source,
        CoreVocabulary::SegmentSelector(operation),
    )
}

const ROWS: &[CoreVocabularyRow] = &[
    row("nat.omg", "Nat", NAT, CoreVocabulary::ProofNatural),
    row(
        "content.omg",
        "Content",
        CONTENT,
        CoreVocabulary::Content(ContentDeclaration::Content),
    ),
    row(
        "content.omg",
        "Content::project",
        CONTENT,
        CoreVocabulary::Content(ContentDeclaration::Project),
    ),
    row(
        "content.omg",
        "IntervalSet",
        CONTENT,
        CoreVocabulary::Content(ContentDeclaration::IntervalSet),
    ),
    row(
        "content.omg",
        "CountedQuantity",
        CONTENT,
        CoreVocabulary::Content(ContentDeclaration::CountedQuantity),
    ),
    row("never.omg", "Never", NEVER, CoreVocabulary::Never),
    row(
        "layout.omg",
        "AccessPolicy",
        LAYOUT,
        CoreVocabulary::AccessPolicy,
    ),
    row(
        "layout.omg",
        "AccessPolicy::plan",
        LAYOUT,
        CoreVocabulary::AccessPolicyPlan,
    ),
    row(
        "layout.omg",
        "AccessPlan",
        LAYOUT,
        CoreVocabulary::AccessPlan,
    ),
    row(
        "assembly.omg",
        "MachineControl",
        ASSEMBLY,
        CoreVocabulary::MachineControlService,
    ),
    row(
        "assembly.omg",
        "PortIo",
        ASSEMBLY,
        CoreVocabulary::PortIoService,
    ),
    selector(
        "code_segment.omg",
        "reload_code_segment",
        CODE_SEGMENT,
        SegmentSelectorOperation::ReloadCode,
    ),
    selector(
        "code_segment.omg",
        "read_code_selector",
        CODE_SEGMENT,
        SegmentSelectorOperation::ReadCode,
    ),
    selector(
        "data_segment.omg",
        "reload_data_segments",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReloadData,
    ),
    selector(
        "data_segment.omg",
        "read_data_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadData,
    ),
    selector(
        "data_segment.omg",
        "read_extra_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadExtra,
    ),
    selector(
        "data_segment.omg",
        "read_stack_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadStack,
    ),
    selector(
        "data_segment.omg",
        "read_fs_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadFs,
    ),
    selector(
        "data_segment.omg",
        "read_gs_selector",
        DATA_SEGMENT,
        SegmentSelectorOperation::ReadGs,
    ),
    row(
        "task_register.omg",
        "activate_task_register",
        TASK_REGISTER,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Activate),
    ),
    row(
        "task_register.omg",
        "read_task_register",
        TASK_REGISTER,
        CoreVocabulary::TaskRegister(TaskRegisterOperation::Read),
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgement,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgement::Pending",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementPending,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementCompletion,
    ),
    row(
        "interrupt.omg",
        "InterruptAcknowledgementCompletion::complete",
        INTERRUPT,
        CoreVocabulary::InterruptAcknowledgementCompletionComplete,
    ),
    row(
        "static_interrupt.omg",
        "StaticInterruptCompletionPermit",
        STATIC_INTERRUPT,
        CoreVocabulary::StaticInterruptCompletionPermit,
    ),
    row(
        "reflect.omg",
        "reflect::schema",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::Schema),
    ),
    row(
        "integer.omg",
        "integer::carrier",
        INTEGER,
        CoreVocabulary::IntegerCarrier(IntegerCarrierDeclaration::Query),
    ),
    row(
        "integer.omg",
        "integer::Carrier",
        INTEGER,
        CoreVocabulary::IntegerCarrier(IntegerCarrierDeclaration::Carrier),
    ),
    row(
        "reflect.omg",
        "reflect::TypeSchema",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::TypeSchema),
    ),
    row(
        "reflect.omg",
        "reflect::callable",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::Callable),
    ),
    row(
        "reflect.omg",
        "reflect::field",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::Field),
    ),
    row(
        "reflect.omg",
        "reflect::parameter",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::Parameter),
    ),
    row(
        "reflect.omg",
        "reflect::ParameterInfo",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::ParameterInfo),
    ),
    row(
        "reflect.omg",
        "reflect::CallableInfo",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::CallableInfo),
    ),
    row(
        "reflect.omg",
        "reflect::CaseInfo",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::CaseInfo),
    ),
    row(
        "reflect.omg",
        "reflect::FieldInfo",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::FieldInfo),
    ),
    row(
        "reflect.omg",
        "reflect::visit_runtime_fields",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFields),
    ),
    row(
        "reflect.omg",
        "reflect::visit_runtime_fields_mut",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitRuntimeFieldsMut),
    ),
    row(
        "reflect.omg",
        "reflect::inspect_record_fields",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::InspectRecordFields),
    ),
    row(
        "reflect.omg",
        "reflect::visit_active_fields",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFields),
    ),
    row(
        "reflect.omg",
        "reflect::visit_active_fields_mut",
        REFLECT,
        CoreVocabulary::Reflection(ReflectionDeclaration::VisitActiveFieldsMut),
    ),
    row(
        "run.omg",
        "Run",
        RUN,
        CoreVocabulary::Run(RunDeclaration::Run),
    ),
    row(
        "run.omg",
        "RunParts",
        RUN,
        CoreVocabulary::Run(RunDeclaration::Parts),
    ),
    // Generic domains retain their own namespace, as Extent's Resident does.
    row(
        "run.omg",
        "Live",
        RUN,
        CoreVocabulary::Run(RunDeclaration::Live),
    ),
    row(
        "run.omg",
        "Singleton",
        RUN,
        CoreVocabulary::Run(RunDeclaration::Singleton),
    ),
    row(
        "layout.omg",
        "ResourceProfile",
        LAYOUT,
        CoreVocabulary::ResourceProfile,
    ),
    row(
        "layout.omg",
        "ResourceProfileSupply",
        LAYOUT,
        CoreVocabulary::ResourceProfileSupply,
    ),
    row(
        "layout.omg",
        "ResourceProfileSupply::profile",
        LAYOUT,
        CoreVocabulary::ResourceProfileSupplyProfile,
    ),
    row("extent.omg", "Extent", EXTENT, CoreVocabulary::Extent),
    row(
        "extent.omg",
        "Extent::Vacant",
        EXTENT,
        CoreVocabulary::ExtentDomain(ExtentDomain::Vacant),
    ),
    // The generic domain `Extent::Resident<P, T>` is declared under its own
    // namespace, so its symbol's path is the bare member name.
    row(
        "extent.omg",
        "Resident",
        EXTENT,
        CoreVocabulary::ExtentDomain(ExtentDomain::Resident),
    ),
    row(
        "extent.omg",
        "Extent::Granted",
        EXTENT,
        CoreVocabulary::ExtentDomain(ExtentDomain::Granted),
    ),
    row(
        "extent.omg",
        "ProgramStorageEntry",
        EXTENT,
        CoreVocabulary::ProgramStorageEntry,
    ),
    row(
        "fixed_vec.omg",
        "FixedVec",
        FIXED_VEC,
        CoreVocabulary::FixedVec,
    ),
    row(
        "assembly_model.omg",
        "port_written",
        ASSEMBLY_MODEL,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::PortWritten),
    ),
    row(
        "assembly_model.omg",
        "model_specific_written",
        ASSEMBLY_MODEL,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::ModelSpecificWritten),
    ),
    row(
        "assembly_model.omg",
        "atomic_release_written",
        ASSEMBLY_MODEL,
        CoreVocabulary::AssemblyEffect(AssemblyEffect::AtomicReleaseWritten),
    ),
    row(
        "external_binding.omg",
        "ForeignBinding",
        EXTERNAL_BINDING,
        CoreVocabulary::ExternalBinding(ExternalBindingVocabulary::ForeignBinding),
    ),
    row(
        "external_binding.omg",
        "DllImport",
        EXTERNAL_BINDING,
        CoreVocabulary::ExternalBinding(ExternalBindingVocabulary::DllImport),
    ),
];

/*
    Bundled core files whose declarations checking recognizes by their own
    hermetic identity rather than by a per-declaration row: the consumer still
    compares the exact identity, and the catalog supplies only that the symbol
    was declared in this exact bundled file. Like every row, a source matches
    only with its exact bytes behind the toolchain-origin gate.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreSource {
    Activation,
    Atomic,
    Relation,
    FloatOperations,
    FloatFormat,
    FloatMeaning,
    Layout,
    Run,
    CodeLayout,
}

const CORE_SOURCES: &[(CoreSource, &str, &[u8])] = &[
    (
        CoreSource::Activation,
        "activation.omg",
        core_source!("activation.omg"),
    ),
    (CoreSource::Atomic, "atomic.omg", core_source!("atomic.omg")),
    (
        CoreSource::Relation,
        "relation.omg",
        core_source!("relation.omg"),
    ),
    (
        CoreSource::FloatOperations,
        "float_operations.omg",
        core_source!("float_operations.omg"),
    ),
    (
        CoreSource::FloatFormat,
        "float_format.omg",
        core_source!("float_format.omg"),
    ),
    (
        CoreSource::FloatMeaning,
        "float_meaning.omg",
        core_source!("float_meaning.omg"),
    ),
    (CoreSource::Layout, "layout.omg", core_source!("layout.omg")),
    (CoreSource::Run, "run.omg", core_source!("run.omg")),
    (
        CoreSource::CodeLayout,
        "code_layout.omg",
        core_source!("code_layout.omg"),
    ),
];

impl CoreSource {
    /// The bundled file's path relative to the core package root.
    pub fn file(self) -> &'static str {
        CORE_SOURCES
            .iter()
            .find(|(source, _, _)| *source == self)
            .map(|(_, file, _)| *file)
            .expect("every core source has a row")
    }

    /// The core source bundled at `file`, if any.
    pub fn from_file(file: &str) -> Option<Self> {
        CORE_SOURCES
            .iter()
            .find(|(_, candidate, _)| *candidate == file)
            .map(|(source, _, _)| *source)
    }
}

pub struct CoreVocabularyCatalog;

impl CoreVocabularyCatalog {
    /// Returns the vocabulary the site's declaration is, or `None` when no row
    /// matches every part of the site.
    pub fn classify(site: SealedDeclarationSite<'_>) -> Option<CoreVocabulary> {
        ROWS.iter()
            .find(|row| {
                site.file == Path::new(row.file)
                    && SealedDeclaration::Exact(row.declaration).matches(site.declaration)
                    && row.source == site.source
            })
            .map(|row| row.vocabulary)
    }

    /// The core source the site's file is, matching path and exact bytes.
    pub fn source(file: &Path, source: &[u8]) -> Option<CoreSource> {
        CORE_SOURCES
            .iter()
            .find(|(_, candidate, bytes)| file == Path::new(candidate) && *bytes == source)
            .map(|(core, _, _)| *core)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site<'a>(
        file: &'a str,
        declaration: &'a str,
        source: &'a [u8],
    ) -> SealedDeclarationSite<'a> {
        SealedDeclarationSite {
            file: Path::new(file),
            declaration,
            source,
        }
    }

    #[test]
    fn proof_natural_recognition_does_not_match_user_or_edited_sources() {
        assert_eq!(
            CoreVocabularyCatalog::classify(site("nat.omg", "Nat", NAT)),
            Some(CoreVocabulary::ProofNatural)
        );
        for candidate in [
            site("user.omg", "Nat", NAT),
            site("nat.omg", "Other", NAT),
            site("nat.omg", "Nat", b"public data Nat {}"),
        ] {
            assert_eq!(CoreVocabularyCatalog::classify(candidate), None);
        }
    }

    #[test]
    fn content_roles_require_the_exact_bundled_declarations() {
        for (name, role) in [
            ("Content", ContentDeclaration::Content),
            ("Content::project", ContentDeclaration::Project),
            ("IntervalSet", ContentDeclaration::IntervalSet),
            ("CountedQuantity", ContentDeclaration::CountedQuantity),
        ] {
            assert_eq!(
                CoreVocabularyCatalog::classify(site("content.omg", name, CONTENT)),
                Some(CoreVocabulary::Content(role))
            );
            for candidate in [
                site("user.omg", name, CONTENT),
                site("content.omg", name, b"edited"),
                site("content.omg", "Lookalike", CONTENT),
            ] {
                assert_eq!(CoreVocabularyCatalog::classify(candidate), None);
            }
        }
    }

    #[test]
    fn profile_roles_require_the_exact_bundled_declarations() {
        for (name, role) in [
            ("ResourceProfile", CoreVocabulary::ResourceProfile),
            (
                "ResourceProfileSupply",
                CoreVocabulary::ResourceProfileSupply,
            ),
            (
                "ResourceProfileSupply::profile",
                CoreVocabulary::ResourceProfileSupplyProfile,
            ),
        ] {
            assert_eq!(
                CoreVocabularyCatalog::classify(site("layout.omg", name, LAYOUT)),
                Some(role)
            );
            for candidate in [
                site("user.omg", name, LAYOUT),
                site("layout.omg", name, b"edited"),
                site("layout.omg", "Lookalike", LAYOUT),
            ] {
                assert_eq!(CoreVocabularyCatalog::classify(candidate), None);
            }
        }
    }

    #[test]
    fn run_roles_require_the_exact_bundled_nominal_declarations() {
        for (name, role) in [
            ("Run", RunDeclaration::Run),
            ("RunParts", RunDeclaration::Parts),
            ("Live", RunDeclaration::Live),
            ("Singleton", RunDeclaration::Singleton),
        ] {
            assert_eq!(
                CoreVocabularyCatalog::classify(site("run.omg", name, RUN)),
                Some(CoreVocabulary::Run(role))
            );
            assert_eq!(
                CoreVocabularyCatalog::classify(site("run.omg", name, b"edited")),
                None
            );
            assert_eq!(
                CoreVocabularyCatalog::classify(site("user.omg", name, RUN)),
                None
            );
            assert_eq!(
                CoreVocabularyCatalog::classify(site("run.omg", &format!("{name}Lookalike"), RUN)),
                None
            );
        }
    }

    #[test]
    fn never_identifies_only_the_exact_bundled_ordinary_declaration() {
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Never", NEVER)),
            Some(CoreVocabulary::Never)
        );
        for candidate in [
            site("other.omg", "Never", NEVER),
            site("never.omg", "Other", NEVER),
            site("never.omg", "Never", b"public data Never {}"),
        ] {
            assert_eq!(CoreVocabularyCatalog::classify(candidate), None);
        }
    }

    #[test]
    fn rows_match_only_their_exact_file_declaration_and_source() {
        assert_eq!(
            CoreVocabularyCatalog::classify(site("assembly.omg", "MachineControl", ASSEMBLY)),
            Some(CoreVocabulary::MachineControlService)
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "data_segment.omg",
                "read_gs_selector",
                DATA_SEGMENT
            )),
            Some(CoreVocabulary::SegmentSelector(
                SegmentSelectorOperation::ReadGs
            ))
        );
        // A selector name in the other segment file is not that primitive.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "code_segment.omg",
                "read_gs_selector",
                CODE_SEGMENT
            )),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("assembly.omg", "MachineControl", b"edited")),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::source(Path::new("atomic.omg"), core_source!("atomic.omg")),
            Some(CoreSource::Atomic)
        );
        assert_eq!(
            CoreVocabularyCatalog::source(Path::new("atomic.omg"), b"edited"),
            None
        );
        assert_eq!(
            CoreSource::from_file(CoreSource::Layout.file()),
            Some(CoreSource::Layout)
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("extent.omg", "Resident", EXTENT)),
            Some(CoreVocabulary::ExtentDomain(ExtentDomain::Resident))
        );
        // `reflect.omg` declares `module reflect`: rows use the full
        // package path, and the bare name matches nothing.
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "reflect.omg",
                "reflect::inspect_record_fields",
                REFLECT
            )),
            Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::InspectRecordFields
            ))
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("reflect.omg", "inspect_record_fields", REFLECT)),
            None
        );
        // The integer carrier query is one exact declaration of `integer.omg`;
        // the family queries built on it are ordinary machines with no row.
        assert_eq!(
            CoreVocabularyCatalog::classify(site("integer.omg", "integer::carrier", INTEGER)),
            Some(CoreVocabulary::IntegerCarrier(
                IntegerCarrierDeclaration::Query
            ))
        );
        for unrecognized in ["carrier", "integer::is_integer", "integer::is_signed"] {
            assert_eq!(
                CoreVocabularyCatalog::classify(site("integer.omg", unrecognized, INTEGER)),
                None,
                "{unrecognized}"
            );
        }
        assert_eq!(
            CoreVocabularyCatalog::classify(site("integer.omg", "integer::carrier", b"edited")),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site(
                "reflect.omg",
                "core::reflect::inspect_record_fields",
                REFLECT
            )),
            None
        );
    }
    #[test]
    fn never_recognition_requires_its_unchanged_core_declaration() {
        let source = core_source!("never.omg");
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Never", source)),
            Some(CoreVocabulary::Never),
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Other", source)),
            None
        );
        assert_eq!(
            CoreVocabularyCatalog::classify(site("never.omg", "Never", b"public data Never {}")),
            None
        );
    }
}
