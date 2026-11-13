/*
    The vocabulary a build-time machine invocation is described in: which
    program and machine to run, with which arguments, selected operators and
    selected target bodies, what filesystem a Build machine may reach, and how
    an invocation that cannot be served is reported.

    These types lived in the tree interpreter (`checked-interpreter`), which
    is removed. They are data, not execution: the Terminal engine identifies
    its retained programs with `BuildMachineProgram`, receipts record
    `SelectedBuildTimeBinaryOperator` and `SelectedBuildTimeMachineBody`, and
    the Build host is configured with `FilesystemAccess`.
*/

/*
    No engine in this crate runs a machine directly on typed trees any more.
    The three jobs that did (a `FixedLayout` policy's `plan`, an access
    policy's plan, and a machine used as a constant domain fact) run before
    checking, so they have no Terminal program to run on yet. Each refuses
    through `no_typed_tree_engine` until it is produced as Terminal Psi
    (TASKS.md, EARLY-TERMINAL-JOBS); the request and result types below are
    the seam that engine fills.
*/

use std::fmt;

use build_time_values::BuildTimeValue;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// The first sentence of the error a compilation reports when a job needs a
/// product path that does not exist yet. The compiler coordinator reports the
/// same sentence for the checked-product path.
pub(crate) const NO_PRODUCT_ROUTE: &str = "no product route yet";

/// The refusal for a machine that only the removed tree interpreter could
/// run. `job` names what the machine was run for.
pub(crate) fn no_typed_tree_engine(job: &str) -> BuildMachineEvaluationError {
    BuildMachineEvaluationError::Rejected(format!(
        "{NO_PRODUCT_ROUTE}: {job} runs a machine at build time before the program is checked, \
         and the tree interpreter that ran it is removed; the program is refused until that \
         machine runs on Terminal Psi, with no claim that it is wrong"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct EvaluationSemanticsIdentity(u32);

impl EvaluationSemanticsIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_SEMANTICS: EvaluationSemanticsIdentity =
    EvaluationSemanticsIdentity(6);

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

impl From<BuildMachineEvaluationError> for String {
    fn from(error: BuildMachineEvaluationError) -> Self {
        error.to_string()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BuildMachineEntry<'a> {
    Name(&'a str),
    Symbol(SymbolHandle),
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

pub struct BuildMachineEvaluationRequest<'a> {
    pub entry: BuildMachineEntry<'a>,
    pub arguments: Vec<BuildTimeValue>,
    pub operators: &'a [SelectedBuildTimeBinaryOperator],
    pub machine_bodies: &'a [SelectedBuildTimeMachineBody],
}

impl<'a> BuildMachineEvaluationRequest<'a> {
    pub fn symbol(machine_symbol: SymbolHandle, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Symbol(machine_symbol),
            arguments,
            operators: &[],
            machine_bodies: &[],
        }
    }
}

#[derive(Clone, Copy)]
pub enum BuildMachineProgram<'program> {
    Typed(&'program TypedTrees),
    Checked(&'program checked_trees::CheckedTrees),
    CheckedClosure {
        execution: &'program checked_trees::CheckedTrees,
        product_declarations: &'program TypedTrees,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct SelectedBuildTimeBinaryOperator {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement: symbols::SymbolHandle,
    pub operation: typed_trees::expression::BinaryOperator,
    pub operands: [typed_trees::expression::ExpressionHandle; 2],
    pub format: numerics::literals::FloatFormat,
    pub policy: checked_trees::CheckedArithmeticPolicyAdapter,
    pub provider: checked_trees::CheckedProviderPlanCommitment,
}

impl SelectedBuildTimeBinaryOperator {
    pub fn prepare(
        program: &typed_trees::TypedTrees,
        expression: typed_trees::expression::ExpressionHandle,
        origin: checked_trees::CheckedValueOrigin,
        requirement: symbols::SymbolHandle,
        policy: checked_trees::CheckedArithmeticPolicyAdapter,
        provider: checked_trees::CheckedProviderPlanCommitment,
    ) -> Result<Self, String> {
        let operator = typed_trees::operator::declaration_by_symbol(program, requirement)
            .ok_or("selected build-time Float requirement disappeared")?;
        let (operation, format) =
            typed_trees::operator::primitive_float_binary_semantics(program, operator)
                .ok_or("selected build-time operation has no matching sealed Float meaning")?;
        let operands = match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Binary(binary) => [binary.left, binary.right],
            typed_trees::expression::ExpressionNode::Call(call) => {
                let [left, right] = program.expression_table.expression_handles(call.arguments)
                else {
                    return Err("selected build-time Float call has wrong arity".into());
                };
                [*left, *right]
            }
            _ => {
                return Err("selected build-time Float expression has no binary operands".into());
            }
        };
        Ok(Self {
            expression,
            origin,
            requirement,
            operation,
            operands,
            format,
            policy,
            provider,
        })
    }

    pub fn has_crash_contract(&self, program: &typed_trees::TypedTrees) -> bool {
        program
            .operators()
            .iter()
            .chain(program.machine_token_bindings())
            .filter(|operator| operator.symbol == self.requirement)
            .any(|operator| {
                program
                    .signature_contracts
                    .span_or_empty(operator.contracts)
                    .iter()
                    .any(|contract| contract.is_crashes())
            })
            || program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == self.requirement)
                .any(|machine| program.machine_declares_crash(machine))
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
