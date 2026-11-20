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
    Dispatch(DispatchError, "dispatch") {
        UncoveredSubject => "dispatch does not cover its subject",
    },
    CrashDeclaration(CrashDeclarationError, "crash-declaration") {
        GuardOnNone => "crashes none cannot carry a route guard",
        UnknownCause => "crash clause requires Trap, Abort or none",
        NoneWithDeclaredCause => "crashes none conflicts with a declared cause",
    },
}
