// Family membership, evidence identities, renderings and the inventory come from one declaration.
macro_rules! typing_reasons {
    ($($family:ident($error:ident, $key:literal) { $($cause:ident => $message:literal),+ $(,)? }),+ $(,)?) => {
        /// Closed subsets of typing refusals; not an exhaustive classification of TypeError.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum TypingReason { $($family($error)),+ }

        $(#[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $error { $($cause),+ }
        impl From<$error> for TypingReason {
            fn from(reason: $error) -> Self { Self::$family(reason) }
        })+

        impl TypingReason {
            pub const ALL: &'static [Self] = &[$($(Self::$family($error::$cause)),+),+];

            pub const fn identity(self) -> &'static str {
                match self { $($(Self::$family($error::$cause) => concat!($key, ":", stringify!($cause))),+),+ }
            }
            pub const fn message(self) -> &'static str {
                match self { $($(Self::$family($error::$cause) => $message),+),+ }
            }
        }
    };
}

typing_reasons! {
    DefinitionCycle(DefinitionCycleError, "definition-cycle") {
        DomainAlias => "cyclic domain alias",
        TypeAliasOrBinderKind => "cyclic type alias or binder kind",
    },
    DirectCallPause(DirectCallPauseError, "direct-call-pause") {
        MissingSuspendAcknowledgement => "call requires a suspend acknowledgement",
        MissingBlockAcknowledgement => "call requires a block acknowledgement",
        ExceedsSuspensionCeiling => "call exceeds the machine's suspension ceiling",
        ExceedsBlockingCeiling => "call exceeds the machine's blocking ceiling",
    },
    CallbackEffect(CallbackEffectError, "callback-effect") {
        OpenReachUnderCeiling => "callback with open service reach cannot fit the machine's reach ceiling",
        ExceedsServiceReach => "callback exceeds the machine's service reach",
        MissingPauseAcknowledgement => "callback requires its declared pause acknowledgement",
        ExceedsPauseCeiling => "callback exceeds the machine's pause ceiling",
    },
    TraitEvidence(TraitEvidenceError, "trait-evidence") {
        WrongTraitApplication => "selected evidence has the wrong trait application",
    },
    OperatorDeclaration(OperatorDeclarationError, "operator-declaration") {
        ConformanceBindsToken => "a conformance supplies the trait's token and cannot bind one",
        MultipleSemanticHomes => "operator declaration has multiple possible semantic homes",
        ForeignHome => "only the declaring package may bind an operator for this home",
        ForeignNominalOperands => "only a nominal operand's declaring package may bind this operator",
        IncorrectArity => "incorrect operator arity",
        PrimitiveFamily => "cannot bind a compiler-owned primitive operator family",
        DuplicateBinding => "duplicate normalized operator binding",
    },
    Dispatch(DispatchError, "dispatch") {
        UncoveredSubject => "dispatch does not cover its subject",
    },
    IndirectCallShape(IndirectCallShapeError, "indirect-call-shape") {
        OperandCountMismatch => "incorrect callback arity",
        NotCallableValue => "indirect call requires a callable value",
        MissingCallableContract => "static binder requires a callable contract",
    },
    LogicalHypothesis(LogicalHypothesisError, "logical-hypothesis") {
        ObservedResult => "logical hypothesis cannot promise an observed Type result",
        MutableAuthority => "logical hypothesis cannot supply mutable authority",
        ExecutableEffect => "logical hypothesis cannot promise executable effects",
    },
    CleanupSelection(CleanupSelectionError, "cleanup-selection") {
        NotExactMachine => "cleanup must select one exact machine",
        ForeignCarrier => "cleanup selection requires the carrier's owning package",
        InvalidSignature => "cleanup must consume exactly one owned carrier and return no value",
        IncompleteDomainApplication => "incomplete cleanup domain application",
        ConflictingSelections => "conflicting cleanup selections on one owned subject",
    },
    CleanupEffect(CleanupEffectError, "cleanup-effect") {
        PublishesCrashRoute => "cleanup cannot publish a crash route",
        MayPause => "cleanup cannot block or suspend",
    },
    CrashDeclaration(CrashDeclarationError, "crash-declaration") {
        GuardOnNone => "crashes none cannot carry a route guard",
        UnknownCause => "crash clause requires Trap, Abort or none",
        NoneWithDeclaredCause => "crashes none conflicts with a declared cause",
    },
}
