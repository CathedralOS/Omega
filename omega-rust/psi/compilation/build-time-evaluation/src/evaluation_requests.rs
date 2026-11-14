//! Real invocation configuration and value data, without an earlier program input.
use build_time_values::BuildTimeValue;
use std::fmt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct EvaluationSemanticsIdentity(u32);
impl EvaluationSemanticsIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum UnavailableStaticInput {
    Binding {
        symbol: symbols::SymbolHandle,
        display: String,
    },
    TypePredicate {
        expression: typed_trees::expression::ExpressionHandle,
        input: symbols::SymbolHandle,
    },
}
impl fmt::Display for UnavailableStaticInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding { display, .. } => write!(
                formatter,
                "static operand `{display}` has no bound local or parameter"
            ),
            Self::TypePredicate { .. } => {
                formatter.write_str("static type predicate remains unbound during execution")
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum BuildMachineEvaluationError {
    StaticInputUnavailable(UnavailableStaticInput),
    Rejected(String),
}
impl fmt::Display for BuildMachineEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaticInputUnavailable(input) => input.fmt(formatter),
            Self::Rejected(reason) => formatter.write_str(reason),
        }
    }
}
impl std::error::Error for BuildMachineEvaluationError {}
impl From<String> for BuildMachineEvaluationError {
    fn from(reason: String) -> Self {
        Self::Rejected(reason)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct SelectedBuildTimeMachineBody {
    pub family: SymbolHandle,
    pub body: SymbolHandle,
}
impl SelectedBuildTimeMachineBody {
    pub fn validate_all(program: &TypedTrees, selections: &[Self]) -> Result<(), String> {
        for (position, selected) in selections.iter().enumerate() {
            let family = program
                .target_machine_families
                .iter()
                .find(|family| family.canonical == selected.family)
                .ok_or_else(|| "build-time body selection names no retained family".to_owned())?;
            if !program
                .machine(selected.family)
                .is_some_and(|machine| machine.target_family_interface)
                || !family
                    .members
                    .iter()
                    .any(|member| member.machine == selected.body)
                || program.machine(selected.body).is_none()
            {
                return Err(
                    "build-time body selection is not a retained member of its exact family"
                        .to_owned(),
                );
            }
            if selections[..position]
                .iter()
                .any(|previous| previous.family == selected.family)
            {
                return Err("build-time family has more than one selected body".to_owned());
            }
        }
        Ok(())
    }
}
/// One private callback slot a layout policy placed while it ran.
#[derive(Debug, Clone, PartialEq)]
pub struct PrivateLayoutPlacementReceipt {
    pub operation_expression: typed_trees::expression::ExpressionHandle,
    pub selected_slot: typed_trees::expression::StaticMachineArgument,
    pub placement: BuildTimeValue,
}
/// What one build-time machine invocation returned: its value and the
/// private layout placements it made on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildTimeOperationEvaluation<T> {
    value: T,
    private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
}
impl<T> BuildTimeOperationEvaluation<T> {
    pub fn new(value: T, private_layout_placements: Vec<PrivateLayoutPlacementReceipt>) -> Self {
        Self {
            value,
            private_layout_placements,
        }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub fn private_layout_placements(&self) -> &[PrivateLayoutPlacementReceipt] {
        &self.private_layout_placements
    }

    pub fn into_value(self) -> T {
        self.value
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedFilesystemRoot {
    pub identity: filesystem_authority::FilesystemGrantRootIdentity,
    pub snapshot: source::SourceSnapshot,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilesystemAccess {
    #[default]
    Virtual,
    RealUnscoped,
    RealScoped(filesystem_authority::FsGrants),
    RealScopedSponsored {
        grants: filesystem_authority::FsGrants,
        sponsor: filesystem_authority::FilesystemSponsor,
    },
    CapturedScoped {
        inputs: Vec<CapturedFilesystemRoot>,
        grants: filesystem_authority::FsGrants,
        sponsor: Option<filesystem_authority::FilesystemSponsor>,
    },
}
impl From<BuildMachineEvaluationError> for String {
    fn from(error: BuildMachineEvaluationError) -> Self {
        error.to_string()
    }
}

pub const CURRENT_EVALUATION_SEMANTICS: EvaluationSemanticsIdentity =
    EvaluationSemanticsIdentity(7);
