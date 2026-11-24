//! Operation names carried between source typing, publication and the Build host.

/// The toolchain protocol whose operations carry a Build selection's
/// designated operands (`Build::bind`, `Build::select_provider`, ...).
pub const SELECTION_PROTOCOL: &str = "BuildBindingOperations";

/// One requirement of the selection protocol, as typing chose it for a
/// generated call. Typing decides the operation from the request it
/// lowers and records it beside that request; publication and the
/// Build host read the recorded operation, so the requirement's
/// declared name is consulted once, here, to find its declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum SelectionProtocolOperation {
    /// Lends the Build value to the activation and returns its ticket.
    Prepare,
    /// Authenticates the Build's `tests` facet before a group selection.
    PrepareTests,
    AwaitProvider,
    /// `Build::bind(builder, slot, entry)` with an authored entry path.
    Bind,
    /// `Build::bind(builder, slot, description)` with an entry value.
    BindDescription,
    BindNative,
    Select,
    SelectProvider,
    SelectProviderOption,
    /// Registers a requirement and lends its activation-owned configuration.
    RegisterTestGroup,
    Exclude,
    DescribeParameter,
    DescribeField,
}

impl SelectionProtocolOperation {
    /// The requirement of `BuildBindingOperations` this operation calls.
    pub const fn requirement_name(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::PrepareTests => "prepare_tests",
            Self::RegisterTestGroup => "register_test_group",
            Self::AwaitProvider => "await_provider",
            Self::Bind => "bind",
            Self::BindDescription => "bind_description",
            Self::BindNative => "bind_native",
            Self::DescribeParameter => "describe_parameter",
            Self::DescribeField => "describe_field",
            Self::Select => "select",
            Self::SelectProvider => "select_provider",
            Self::SelectProviderOption => "select_provider_option",
            Self::Exclude => "exclude",
        }
    }

    /// Whether this is the call that lends the Build before a commit.
    pub const fn prepares(self) -> bool {
        matches!(self, Self::Prepare | Self::PrepareTests)
    }
}
