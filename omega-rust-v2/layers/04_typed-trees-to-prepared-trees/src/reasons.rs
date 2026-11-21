//! The closed set of rules by which preparation itself refuses a source.
//!
//! Every refusal L4 renders into a `TypeError` carries the typed cause it was
//! rendered from. The cause is either one `PreparationReason` this layer owns
//! or a reason forwarded unchanged from the target contract or the shared
//! integer primitive. `PreparationReason::ALL` enumerates the owned rules, so a
//! report can compare the declared inventory with the reasons tests observed.
//! Typing refusals before or after evaluation remain L3's diagnostics and have
//! no preparation cause.
use omega_typed::{Site, TypeError};

/// Rendered before the reason text of every refusal at an evaluated occurrence.
pub(crate) const ADMISSION_PREFIX: &str = "static evaluation cannot admit this invocation: ";

/// Declares each reason once so `ALL` and `text` cannot omit a variant.
macro_rules! preparation_reasons {
    ($($(#[$doc:meta])* $kind:ident => $text:literal,)*) => {
        /// A rule by which preparation refuses to evaluate or freeze a value.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub enum PreparationReason {
            $($(#[$doc])* $kind,)*
        }

        impl PreparationReason {
            /// Every owned reason, in declaration order.
            pub const ALL: &'static [PreparationReason] = &[$(PreparationReason::$kind,)*];

            /// The reason as rendered in the diagnostic. Most diagnostics place it
            /// after the admission prefix and may append a call chain or recursive
            /// component; see `RefusalCause` for the exceptions.
            pub const fn text(self) -> &'static str {
                match self {
                    $(PreparationReason::$kind => $text,)*
                }
            }
        }
    };
}

preparation_reasons! {
    // Preparation rounds.
    /// The source package's instruction catalog and the supplied layout target differ. Rendered without the admission prefix at an empty site.
    AssemblyTargetMismatch => "assembly catalog and preparation selected different targets",
    /// Typing published the same demands after receiving answers for them.
    TypingDidNotAdvance => "typing did not advance after receiving the evaluation result",

    // Static demands and named constants.
    /// A static index value has no canonical identity.
    NoCanonicalIndexIdentity => "the prepared value has no canonical static index identity",
    /// A named constant read has no typed initializer in this pass.
    ConstantWithoutInitializer => "the selected constant has no local typed initializer",
    /// A typed initializer selects an expression other than its declaration's RHS.
    ConstantInitializerDeclarationMismatch => "the selected expression is not the initializer of this constant declaration",
    /// The pass contains more than one initializer record for the same declaration.
    DuplicateConstantInitializer => "the constant declaration has more than one typed initializer record",
    /// Named constant initializers read each other in a cycle.
    CyclicConstantInitializer => "cyclic constant initializer",
    /// A constant reference selects more than one declaration.
    AmbiguousConstantReference => "ambiguous constant reference",
    /// A named constant value or read requires a different carrier from its initializer.
    ConstantCarrierMismatch => "the selected constant has a different carrier",
    /// At publication, the evaluated integer is outside its initializer's numeric range.
    FrozenConstantOutsideCarrier => "the frozen constant exceeds its carrier",

    /// L3 selected the declaration, but preparation cannot freeze its description into retained storage.
    DescriptorStorageUnavailable => "freezing a reflected declaration requires descriptor storage",

    // Integer primitives.
    /// A trapping division or remainder has a zero divisor.
    ZeroDivisorTrapPossible => "a zero divisor leaves the selected Trap route possible",
    /// An exact division or remainder has a zero divisor.
    ZeroDivisor => "integer division or remainder requires a nonzero divisor",
    /// A trapping quotient cannot be represented.
    QuotientTrapPossible => "an unrepresentable quotient leaves the selected Trap route possible",
    /// An exact remainder's quotient cannot be represented.
    RemainderQuotientOutsideCarrier => "exact remainder requires a representable integer quotient",
    /// A trapping operation overflows its carrier.
    OverflowTrapPossible => "overflow leaves the selected Trap route possible",
    /// Exact addition leaves its carrier.
    AdditionOutsideCarrier => "exact addition exceeds its selected integer carrier",
    /// Exact subtraction leaves its carrier.
    SubtractionOutsideCarrier => "exact subtraction exceeds its selected integer carrier",
    /// Exact multiplication leaves its carrier.
    MultiplicationOutsideCarrier => "exact multiplication exceeds its selected integer carrier",
    /// Exact division leaves its carrier.
    DivisionOutsideCarrier => "exact division exceeds its selected integer carrier",
    /// Another exact integer operation leaves its carrier.
    OperationOutsideCarrier => "exact operation exceeds its selected integer carrier",
    /// The selected integer primitive has no execution rule.
    IntegerPrimitiveWithoutRule => "the selected integer primitive has no execution rule",
    /// A typed landing is not an integer.
    LandingNotIntegral => "the typed landing does not supply an integral value",
    /// A typed landing does not fit its carrier.
    LandingOutsideCarrier => "the typed landing exceeds its carrier",
    /// One occurrence lands two different integers.
    ConflictingIntegerLandings => "conflicting integer landings at one typed occurrence",

    // Floating point.
    /// One occurrence lands two different floating values.
    ConflictingFloatLandings => "conflicting floating landings at one typed occurrence",
    /// A floating operation runs before a target is selected. Reported as missing target input.
    FloatNeedsTargetRealization => "the floating operation needs a selected target realization",
    /// The selected target has no realization for the operation. Rendered without the admission prefix, followed by ` (<format> <operation> on <profile>)`.
    UnsupportedFloatOperation => "the selected target does not support this floating operation",
    /// A floating carrier cast changes the value.
    InexactFloatCast => "the floating carrier cast does not preserve the exact value",

    // Evaluation fragment.
    /// The selected body is outside the evaluated fragment.
    UnsupportedBody => "the selected body is outside the supported static evaluation fragment",
    /// A static call argument has no closed value in the evaluated fragment.
    UnsupportedArgument => "the static call has an argument without a supported closed value",
    /// A closed generic argument does not satisfy its parameter's storage contract.
    MultiplicityBoundMismatch => "the selected type argument does not satisfy its multiplicity bound",
    /// A contract needs proof outside the evaluated fragment.
    UnsupportedContract => "the selected contract needs proof outside the supported static evaluation fragment",
    /// A request's typed source file is not in this pass.
    RequestWithoutTypedFile => "the evaluation request has no typed source file",
    /// The source file exists but does not contain the requested expression node.
    RequestWithoutTypedNode => "the evaluation request has no expression at the selected node",
    /// The operand node and its published source occurrence disagree.
    OperandSiteMismatch => "the call operand selects a different source occurrence",
    /// The request names an existing expression at a different source occurrence.
    RequestSiteMismatch => "the evaluation request selects a different source occurrence",
    /// A pattern selects no supported owned value.
    PatternWithoutSelection => "the selected pattern has no supported owned-value selection",
    /// A fixed-array read is out of bounds.
    ArrayIndexOutOfBounds => "selected array read requires index < items.len",
    /// A zero-filled array cannot be allocated.
    ZeroArrayNeedsStorage => "zero-filling an array requires checked preparation storage",

    // Layout queries.
    /// A layout query call is not one core query.
    NotOneLayoutQuery => "the request does not select one core layout query",
    /// A layout query's subject is not closed.
    LayoutQueryNeedsClosedType => "the layout query needs a closed type application",
    /// A layout query runs before a target is selected. Reported as missing target input.
    LayoutQueryNeedsTarget => "the layout query requires a selected target",

    // Invocation admission.
    /// A runtime boundary slot is selected during preparation.
    UnselectedBoundarySlot => "the selected runtime boundary slot cannot execute during preparation",
    /// Two boundary slots match one invocation.
    CompetingBoundarySlots => "the selected invocation has competing runtime boundary slots",
    /// The typed and supplied boundary slots differ.
    BoundarySlotSelectionsDiffer => "the selected runtime boundary slot cannot execute under different build selections",
    /// The request does not select one ordinary static invocation.
    NotOneOrdinaryInvocation => "the request does not select one ordinary static invocation",
    /// The selected declaration has no typed signature.
    DeclarationWithoutSignature => "the selected declaration has no typed signature",
    /// The selected declaration has no local body.
    DeclarationWithoutBody => "the selected declaration has no local body",
    /// An exported machine without a termination promise is invoked.
    PublicMachineWithoutTermination => "the selected public machine does not declare termination",
    /// The selected body has no complete effect row.
    BodyWithoutEffectRow => "the selected body has no complete typed effect row",
    /// The selected body reaches services, suspends or blocks.
    InvocationNotEffectFree => "the selected invocation is not effect-free during preparation",
    /// The typed call and the selected parameters differ in arity.
    CallParametersDisagree => "the typed call and selected parameters disagree",
    /// A closed argument does not fit its parameter carrier.
    ArgumentOutsideCarrier => "a closed argument is outside its selected carrier",
    /// A Boolean-result invocation's precondition is false.
    BooleanPreconditionFalse => "the selected Boolean precondition is false for the supplied arguments",
    /// An integer-result invocation's precondition is false.
    IntegerPreconditionFalse => "the selected integer precondition is false for the supplied arguments",
    /// Another invocation's precondition is false.
    PreconditionFalse => "the selected precondition is false for the supplied arguments",
    /// The signature and its selected declaration disagree about an ensures clause.
    PostconditionDeclarationMismatch => "the signature and declaration disagree about postconditions",
    /// A postcondition has no typed proposition.
    PostconditionWithoutProposition => "the selected postcondition has no typed proposition",
    /// A declared crash cause has no typed route.
    CrashCauseWithoutRoute => "the selected crash cause has no typed route",
    /// A Trap route's guard holds for the operands.
    TrapRoutePossible => "the selected Trap route remains possible for the supplied arguments",
    /// An Abort route's guard holds for the operands.
    AbortRoutePossible => "the selected Abort route remains possible for the supplied arguments",
    /// A crash guard has no typed predicate.
    CrashGuardWithoutPredicate => "the selected crash guard has no typed predicate",
    /// A static call operand is not a local scalar.
    NoLocalScalarOperand => "the static call does not have a local scalar operand",
    /// A parameter carrier is outside the bounded integer fragment.
    ParameterOutsideIntegerFragment => "a selected parameter is outside the bounded integer fragment",

    // Checked invocation proofs.
    /// A precondition needs a checked proof.
    PreconditionNeedsProof => "the selected precondition requires a checked proof for the supplied arguments",
    /// A Trap route needs a checked disproof.
    TrapRouteNeedsDisproof => "the selected Trap route requires a checked disproof for the supplied arguments",
    /// An Abort route needs a checked disproof.
    AbortRouteNeedsDisproof => "the selected Abort route requires a checked disproof for the supplied arguments",
    /// A postcondition needs a checked proof.
    PostconditionNeedsProof => "the selected postcondition requires a checked proof for the arguments and result",
    /// The only applicable invocation proof observed another target.
    InvocationProofTargetChanged => "the checked invocation proof depends on a different target selection",

    // Providers.
    /// The provider does not certify the selected conformance binding.
    ProviderConformanceMismatch => "the checked provider does not certify the selected conformance binding",
    /// The provider does not match the selected target plan.
    ProviderTargetPlanMismatch => "the checked provider does not match the selected target plan",
    /// The provider proof observed another target.
    ProviderProofTargetChanged => "the checked provider proof depends on a different target selection",
    /// The provider does not preserve the positional application.
    ProviderApplicationMismatch => "the checked provider does not preserve the selected positional application",

    // Static callbacks.
    /// A callable refinement has no closed caller application.
    RefinementWithoutCaller => "the static callable refinement has no closed caller application",
    /// A callable refinement names another application.
    RefinementApplicationMismatch => "the static callable refinement does not identify the selected application",
    /// A static callback has no typed signature.
    CallbackWithoutSignature => "the static callback has no typed signature",
    /// A static callback has no typed bound.
    CallbackWithoutBound => "the static callback has no typed bound",
    /// A callable use does not select one exact callback.
    InexactCallbackSelection => "preparation requires an exact static callback selection",
    /// A static callback has no closed caller application.
    CallbackWithoutCallerApplication => "the static callback has no closed caller application",
    /// The selected callback differs from the typed call.
    CallbackDisagreesWithCall => "the selected static callback disagrees with the typed call",
    /// A callable argument is a runtime value.
    RuntimeCallableArgument => "preparation cannot materialize a runtime callable argument",
    /// A callable bound's guarantee needs a checked judgment.
    CallableGuaranteeNeedsJudgment => "the callable bound's guarantee needs a checked judgment for the arguments and result",
    /// A callable bound's precondition needs a checked judgment.
    CallablePreconditionNeedsJudgment => "the callable bound's precondition needs a checked judgment for the supplied arguments",
    /// An exact callback needs a checked refinement of its callable contract.
    CallbackNeedsRefinement => "the exact static callback needs a checked refinement of its callable contract",
    /// A forwarded callback needs a checked refinement of its callable contract.
    ForwardedCallbackNeedsRefinement => "the exact static callback needs checked refinement of its callable contract",
    /// A cyclic callback dependency has no termination proof.
    CyclicCallbackWithoutTermination => "a cyclic static callback dependency has no checked termination proof",
    /// The callback's contract needs evidence outside the evaluated fragment.
    CallbackContractOutsideFragment => "the exact static callback needs checked contract evidence outside the supported fragment",
    /// A runtime boundary selection is used as a static callback.
    RuntimeBoundaryCallback => "a runtime boundary selection cannot execute as a static callback",
    /// The callback has no complete operational bounds.
    CallbackWithoutOperationalBounds => "the exact static callback has no complete operational bounds",
    /// The callback's operational effects are unresolved or nonempty.
    CallbackEffectsNotEmpty => "the exact static callback has unresolved or nonempty operational effects",
    /// A callable formal has no closed selection.
    CallbackWithoutSelection => "the static callback has no closed selection",

    // Termination.
    /// A cyclic invocation has no checked termination proof.
    CyclicInvocation => "a cyclic static invocation requires a checked termination proof",
    /// A callback's termination closure contains an unresolved call.
    UnresolvedCallbackClosure => "the exact static callback has an unresolved call in its termination closure",
    /// The termination proof observed another target.
    TerminationProofTargetChanged => "the checked termination proof depends on a different target selection",
    /// A ranked body has no checked termination proof.
    RankedBodyNeedsProof => "the selected ranked body requires a checked termination proof",

    // Owned values.
    /// A constructor needs data or canonical proof outside the prepared fragment.
    ConstructionNeedsProof => "the selected constructor requires proof outside the prepared value fragment",
    /// A field projection names a field outside the active value.
    FieldNotInActiveValue => "the selected field is not in the active owned value",
    /// A nominal value has no supported initializer.
    NominalWithoutInitializer => "the selected nominal value has no supported initializer",
    /// A constructor builds another carrier.
    ConstructorCarrierMismatch => "the selected constructor has a different carrier",
    /// A case does not belong to the carrier.
    CaseOutsideCarrier => "the selected case does not belong to the carrier",
    /// An authored initializer points outside its construction's subtree.
    InitializerOutsideConstruction => "the selected field initializer is outside its construction",
    /// An authored field position is absent or already occupied in the selected shape.
    InvalidConstructorFieldSlot => "the selected field initializer has no unique position in this construction",
    /// A constructor field has neither initializer nor typed zero request.
    ConstructorFieldWithoutInitializer => "the selected constructor has a field without an initializer or typed zero request",
    /// A by-value type contains itself.
    RecursiveByValueType => "a recursive by-value type has no supported frozen value",
    /// The complete array element type does not permit copying.
    ArrayElementNotCopyable => "the complete array element type must permit copying",
    /// The complete constant type does not permit copying.
    ConstantTypeNotCopyable => "the complete constant type must permit copying",
    /// The complete constant type has a cleanup obligation.
    ConstantTypeHasCleanup => "the complete constant type has a cleanup obligation",
    /// The complete constant type has no finite field shape.
    ConstantTypeWithoutFieldShape => "the complete constant type has no finite typed field shape",
    /// The complete array element type is outside the prepared fragment.
    ArrayElementOutsideFragment => "the complete array element type is outside the prepared value fragment",
    /// The complete constant type is outside the prepared fragment.
    ConstantTypeOutsideFragment => "the complete constant type is outside the prepared value fragment",
    /// An owned field read has another result type or source tree.
    OwnedFieldMismatch => "the selected owned field has a different result type or source tree",
    /// A field read needs proof or borrowed storage.
    FieldReadNeedsProof => "the selected field read requires proof or borrowed storage",
    /// An owned field read has no supported root type.
    OwnedFieldWithoutRootType => "the selected owned field has no supported root type",
    /// A tuple has no matching typed initializer.
    TupleWithoutInitializer => "the selected tuple has no matching typed initializer",
    /// A byte literal has another array type.
    ByteLiteralTypeMismatch => "the selected byte literal has a different array type",
    /// An array value has no supported initializer.
    ArrayWithoutInitializer => "the selected array value has no supported initializer",
    /// An omitted field's zero value needs proof of the data predicate.
    OmittedFieldZeroNeedsProof => "the omitted field's zero value requires proof of the data predicate",
}

impl std::fmt::Display for PreparationReason {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// The typed cause of one refusal L4 rendered into a `TypeError`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RefusalCause {
    /// A rule this layer owns.
    Preparation(PreparationReason),
    /// The target contract's layout projection refused the query. The text is rendered unchanged.
    TargetLayout(&'static str),
    /// The shared integer primitive refused the operation. The text is rendered unchanged.
    IntegerPrimitive(&'static str),
}

impl RefusalCause {
    /// The owned rule, when this layer owns the cause.
    pub const fn reason(self) -> Option<PreparationReason> {
        match self {
            Self::Preparation(reason) => Some(reason),
            Self::TargetLayout(_) | Self::IntegerPrimitive(_) => None,
        }
    }

    pub const fn text(self) -> &'static str {
        match self {
            Self::Preparation(reason) => reason.text(),
            Self::TargetLayout(text) | Self::IntegerPrimitive(text) => text,
        }
    }
}

impl From<PreparationReason> for RefusalCause {
    fn from(reason: PreparationReason) -> Self {
        Self::Preparation(reason)
    }
}

impl std::fmt::Display for RefusalCause {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// A rendered preparation diagnostic and the cause it was rendered from.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparationRefusal {
    pub diagnostic: TypeError,
    pub cause: RefusalCause,
}

impl PreparationRefusal {
    /// Render a refusal at an evaluated occurrence.
    pub(crate) fn at(
        site: &Site,
        cause: impl Into<RefusalCause>,
    ) -> Self {
        let cause = cause.into();
        Self {
            diagnostic: TypeError::new(&site.package, &site.file, site.span, format!("{ADMISSION_PREFIX}{}", cause.text())),
            cause,
        }
    }

    /// Keep a diagnostic whose rendering does not use the admission prefix.
    pub(crate) fn rendered(
        diagnostic: TypeError,
        reason: PreparationReason,
    ) -> Self {
        Self {
            diagnostic,
            cause: RefusalCause::Preparation(reason),
        }
    }

    pub fn into_diagnostic(self) -> TypeError {
        self.diagnostic
    }
}

impl From<PreparationRefusal> for TypeError {
    fn from(refusal: PreparationRefusal) -> Self {
        refusal.diagnostic
    }
}
