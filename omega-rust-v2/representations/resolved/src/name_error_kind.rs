//! The semantic rule a name-resolution failure violates. Every `NameError`
//! carries exactly one kind, so a report can enumerate the complete set of
//! L2 refusals from `NameErrorKind::ALL` and group them by `NameErrorOrigin`.

/// Who supplied the input that a refusal rejects.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum NameErrorOrigin {
    /// The stage admission: parsed units, their package identities and logical paths, and dependency edges.
    StageInput,
    /// A compiled interface supplied in place of a dependency's source.
    CompiledInterface,
    /// The selected target's assembly vocabulary does not admit a name the source uses.
    Target,
    /// An author can write the refusal in Omega source.
    Source,
}

/// Declares the kinds once so `ALL` and `origin` cannot omit a variant.
macro_rules! name_error_kinds {
    ($($(#[$doc:meta])* $kind:ident => $origin:ident,)*) => {
        /// The semantic rule a name-resolution failure violates.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub enum NameErrorKind {
            $($(#[$doc])* $kind,)*
        }

        impl NameErrorKind {
            /// Every kind, in declaration order.
            pub const ALL: &'static [NameErrorKind] = &[$(NameErrorKind::$kind,)*];

            pub const fn origin(self) -> NameErrorOrigin {
                match self {
                    $(NameErrorKind::$kind => NameErrorOrigin::$origin,)*
                }
            }
        }
    };
}

name_error_kinds! {
    /// A file's tokens and syntax tree do not index its source text.
    InconsistentParsedSource => StageInput,
    /// Two admitted files share a package and logical path.
    DuplicateLogicalFile => StageInput,
    /// A logical path is not a package-relative file below identifier directories.
    InvalidLogicalPath => StageInput,
    /// One requester maps a dependency alias to two packages.
    ConflictingDependencyAlias => StageInput,
    /// A source package claims the toolchain's package identity.
    ReservedPackageIdentity => StageInput,

    /// A compiled interface describes a package that is also admitted as source.
    CompiledPackageCollision => CompiledInterface,
    /// A compiled export has a declaration kind that interfaces cannot carry.
    UnsupportedCompiledDeclaration => CompiledInterface,
    /// A compiled export names a nested machine or a declaration local to one.
    NestedCompiledExport => CompiledInterface,
    /// A compiled export's module is not among the interface's modules.
    CompiledExportWithoutModule => CompiledInterface,
    /// A compiled export's path is not inside its defining module.
    CompiledExportOutsideModule => CompiledInterface,
    /// A compiled domain's carrier disagrees with whether its declaration is attached.
    CompiledDomainAttachmentMismatch => CompiledInterface,
    /// A compiled interface exports one declaration identity twice.
    DuplicateCompiledDeclaration => CompiledInterface,
    /// A compiled module path has a segment that is not a module name.
    InvalidCompiledModulePath => CompiledInterface,
    /// A compiled export other than a trait lists parent traits.
    CompiledParentsOnNonTrait => CompiledInterface,
    /// A compiled trait's parent is not a visible public trait.
    UnknownCompiledParentTrait => CompiledInterface,
    /// Compiled trait parents form a cycle that reaches a compiled trait first.
    CyclicCompiledTraitInheritance => CompiledInterface,
    /// Two compiled members on one owner share a name.
    DuplicateCompiledMember => CompiledInterface,

    /// An instruction mnemonic is not admitted by the target vocabulary.
    UnavailableTargetInstruction => Target,
    /// A register is not admitted by the target vocabulary.
    UnavailableTargetRegister => Target,

    /// A parameter, local or pattern binding reuses the name of a visible local value.
    ShadowedValueBinding => Source,
    /// One generic or runtime parameter list names a parameter twice.
    DuplicateParameter => Source,
    /// A data, trait or generic type binder redeclares a builtin type spelling.
    BuiltinTypeRedeclared => Source,
    /// A pattern inside a subject predicate rebinds `self`.
    PredicateSubjectRebound => Source,
    /// A domain head index is not one of the domain's own binders.
    DomainIndexNotOwnBinder => Source,
    /// A case, nested data or attached domain reuses a member name of its owner.
    DuplicateMember => Source,
    /// Two source declarations have the same identity after their names bind.
    DuplicateDeclaration => Source,
    /// A name, qualified path or attached-domain carrier selects no visible declaration or binding.
    UnresolvedName => Source,
    /// An attached domain's carrier names more than one data or domain declaration.
    AmbiguousCarrier => Source,
    /// A trait parent names no visible trait.
    UnresolvedParentTrait => Source,
    /// A trait parent names more than one trait.
    AmbiguousParentTrait => Source,
    /// Source trait parents form a cycle.
    CyclicTraitInheritance => Source,
    /// A domain's carrier belongs to a package the requester has no authority to name.
    CarrierWithoutAuthority => Source,
    /// The root of a path denotes two modules.
    CompetingModulePaths => Source,
    /// An import selects neither a module nor an accessible declaration.
    UnavailableImport => Source,
    /// A name selects declarations that are not one overload set.
    CompetingDeclarations => Source,
    /// A static member root has more than one eligible owner.
    AmbiguousStaticRoot => Source,
    /// A module machine's directory qualifier names no module.
    UnresolvedModuleOwner => Source,
    /// A qualified machine head names a machine, data carrier or other declaration that cannot own it.
    IneligibleMachineOwner => Source,
    /// A qualified machine head's owner names nothing.
    UnresolvedMachineOwner => Source,
    /// A qualified machine head's owner names more than one declaration.
    AmbiguousMachineOwner => Source,
    /// An assembly transfer names no local assembly block.
    UnresolvedAssemblyBlock => Source,
    /// An assembly transfer names more than one local assembly block.
    AmbiguousAssemblyBlock => Source,
    /// A public signature names a private declaration.
    PrivateDeclarationInPublicSignature => Source,
    /// An applied qualifier selects no owner of the resolved name.
    UnresolvedApplicationOwner => Source,
    /// A conformance does not name exactly one trait.
    ConformanceHeadNotTrait => Source,
    /// A selection row's target names no machine.
    SelectionRowNotMachine => Source,
    /// A selection row names a requirement outside the implemented trait and its parents.
    RequirementOfDifferentTrait => Source,
}
