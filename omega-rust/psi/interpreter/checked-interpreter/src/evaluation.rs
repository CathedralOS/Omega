/*
    Measured checked-tree evaluation outcomes, build-time value observation,
    sponsor accounting, and virtual clock bindings.

    We record step schedules, storage cell and text-byte watermarks, output
    obligations, and executed behavior exclusions alongside the pure build-time
    value and virtual wait vocabulary.
*/

use crate::FilesystemGrantRootIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct EvaluationSemanticsIdentity(u32);

impl EvaluationSemanticsIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_SEMANTICS: EvaluationSemanticsIdentity =
    EvaluationSemanticsIdentity(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct EvaluationStepScheduleIdentity(u32);

impl EvaluationStepScheduleIdentity {
    pub const fn marker(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_STEP_SCHEDULE: EvaluationStepScheduleIdentity =
    EvaluationStepScheduleIdentity(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct EvaluationUsageSchemaIdentity(u32);

impl EvaluationUsageSchemaIdentity {
    pub const fn schema_version(self) -> u32 {
        self.0
    }
}

pub const CURRENT_EVALUATION_USAGE_SCHEMA: EvaluationUsageSchemaIdentity =
    EvaluationUsageSchemaIdentity(7);

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct EvaluationUsage {
    pub(crate) schema: EvaluationUsageSchemaIdentity,
    pub(crate) schedule: EvaluationStepScheduleIdentity,
    pub(crate) work_units: u64,
    pub(crate) work_budget: u64,
    pub(crate) build_log_bytes: u64,
    pub(crate) filesystem_operation_attempts: u64,
    pub(crate) peak_live_cells: u64,
    pub(crate) peak_live_text_bytes: u64,
    pub(crate) result_cells: u64,
    pub(crate) result_text_bytes: u64,
}

pub use filesystem_authority::{
    BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt,
    MAX_BUILD_OUTPUT_OBLIGATIONS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIncludedSource {
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
    pub(crate) filesystem_attempt_ordinal: usize,
}

impl BuildIncludedSource {
    pub(crate) fn new(
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
        filesystem_attempt_ordinal: usize,
    ) -> Self {
        Self {
            root,
            relative_path,
            filesystem_attempt_ordinal,
        }
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn filesystem_attempt_ordinal(&self) -> usize {
        self.filesystem_attempt_ordinal
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedBuildSelection {
    pub operation: language_semantics::declaration_selection::BuildOperation,
    pub machine: symbols::SymbolHandle,
    pub expression: typed_trees::expression::ExpressionHandle,
    pub composition_case: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedBehaviorExclusion {
    pub machine: symbols::SymbolHandle,
    pub site: ExecutedBehaviorExclusionSite,
    pub kind: ExecutedBehaviorExclusionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutedBehaviorExclusionSite {
    Statement(typed_trees::statement::StatementHandle),
    Expression(typed_trees::expression::ExpressionHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutedBehaviorExclusionKind {
    CrashCause { case_symbol: symbols::SymbolHandle },
    PhysicalAuthorityClass { case_symbol: symbols::SymbolHandle },
    Service,
}

impl EvaluationUsage {
    pub(crate) const fn empty(work_budget: u64) -> Self {
        Self {
            schema: CURRENT_EVALUATION_USAGE_SCHEMA,
            schedule: CURRENT_EVALUATION_STEP_SCHEDULE,
            work_units: 0,
            work_budget,
            build_log_bytes: 0,
            filesystem_operation_attempts: 0,
            peak_live_cells: 0,
            peak_live_text_bytes: 0,
            result_cells: 0,
            result_text_bytes: 0,
        }
    }

    pub const fn schedule(self) -> EvaluationStepScheduleIdentity {
        self.schedule
    }

    pub const fn schema(self) -> EvaluationUsageSchemaIdentity {
        self.schema
    }

    pub const fn work_units(self) -> u64 {
        self.work_units
    }

    pub const fn work_budget(self) -> u64 {
        self.work_budget
    }

    pub const fn build_log_bytes(self) -> u64 {
        self.build_log_bytes
    }

    pub const fn filesystem_operation_attempts(self) -> u64 {
        self.filesystem_operation_attempts
    }

    pub const fn peak_live_cells(self) -> u64 {
        self.peak_live_cells
    }

    pub const fn peak_live_text_bytes(self) -> u64 {
        self.peak_live_text_bytes
    }

    pub const fn result_cells(self) -> u64 {
        self.result_cells
    }

    pub const fn result_text_bytes(self) -> u64 {
        self.result_text_bytes
    }

    pub(crate) fn charge_step(&mut self) -> Option<()> {
        self.work_units = self.work_units.checked_add(1)?;
        Some(())
    }

    pub(crate) fn set_work_budget(&mut self, work_budget: u64) {
        self.work_budget = work_budget;
    }

    pub(crate) fn charge_build_log_bytes(&mut self, bytes: u64) -> Option<()> {
        self.build_log_bytes = self.build_log_bytes.checked_add(bytes)?;
        Some(())
    }

    pub(crate) fn charge_filesystem_operation_attempt(&mut self) -> Option<()> {
        self.filesystem_operation_attempts = self.filesystem_operation_attempts.checked_add(1)?;
        Some(())
    }

    pub(crate) fn record_peak_live_cells(&mut self, peak_live_cells: u64) {
        self.peak_live_cells = peak_live_cells;
    }

    pub(crate) fn record_peak_live_text_bytes(&mut self, peak_live_text_bytes: u64) {
        self.peak_live_text_bytes = peak_live_text_bytes;
    }

    pub(crate) fn record_result_custody(&mut self, result_cells: u64, result_text_bytes: u64) {
        self.result_cells = result_cells;
        self.result_text_bytes = result_text_bytes;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    pub(crate) value: T,
    pub(crate) usage: EvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub(crate) fn new(value: T, usage: EvaluationUsage) -> Self {
        Self { value, usage }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> EvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, EvaluationUsage) {
        (self.value, self.usage)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrivateLayoutPlacementReceipt {
    pub operation_expression: typed_trees::expression::ExpressionHandle,
    pub selected_slot: typed_trees::expression::StaticMachineArgument,
    pub placement: build_time_values::BuildTimeValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildTimeOperationEvaluation<T> {
    pub(crate) measured: MeasuredEvaluation<T>,
    pub(crate) private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
}

impl<T> BuildTimeOperationEvaluation<T> {
    pub(crate) fn new(
        value: T,
        usage: EvaluationUsage,
        private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
    ) -> Self {
        Self {
            measured: MeasuredEvaluation::new(value, usage),
            private_layout_placements,
        }
    }

    pub const fn value(&self) -> &T {
        self.measured.value()
    }

    pub const fn usage(&self) -> EvaluationUsage {
        self.measured.usage()
    }

    pub fn private_layout_placements(&self) -> &[PrivateLayoutPlacementReceipt] {
        &self.private_layout_placements
    }

    pub fn into_parts(self) -> (T, EvaluationUsage, Vec<PrivateLayoutPlacementReceipt>) {
        let (value, usage) = self.measured.into_parts();
        (value, usage, self.private_layout_placements)
    }

    pub fn into_measured(self) -> MeasuredEvaluation<T> {
        self.measured
    }

    pub fn into_value(self) -> T {
        self.measured.into_value()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretOutcome {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
    pub crash_cause: Option<typed_trees::signature::CrashCause>,
    pub usage: EvaluationUsage,
}

impl InterpretOutcome {
    pub(crate) fn exited(
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            error: None,
            crash_cause: None,
            usage,
        }
    }

    pub(crate) fn error(
        message: impl Into<String>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr,
            error: Some(message.into()),
            crash_cause: None,
            usage,
        }
    }

    pub(crate) fn crashed(
        cause: typed_trees::signature::CrashCause,
        site: String,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        usage: EvaluationUsage,
    ) -> Self {
        let mut outcome = Self::error(
            format!("explicit crash {cause:?} at {site}"),
            stdout,
            stderr,
            usage,
        );
        outcome.crash_cause = Some(cause);
        outcome
    }
}

pub const MAX_INCLUDED_BUILD_SOURCES: usize = 256;

pub(crate) mod build_evaluation_sponsor {
    pub use evaluation_accounting::{
        BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION, BuildEvaluationLiveCellLease,
        BuildEvaluationLiveFilesystemHandleLease, BuildEvaluationLiveTextByteLease,
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
    };
}

pub(crate) mod build_evaluation_usage {
    use crate::{BuildEvaluationSponsor, EvaluationUsage};
    use evaluation_accounting::{
        BuildEvaluationUsage, BuildEvaluationUsageError, EvaluationMeasurementIdentity,
    };

    pub fn capture_checked_build_evaluation_usage(
        usage: EvaluationUsage,
        sponsor: Option<&BuildEvaluationSponsor>,
    ) -> Result<BuildEvaluationUsage, BuildEvaluationUsageError> {
        if sponsor.is_some_and(|sponsor| {
            sponsor.measurement() != EvaluationMeasurementIdentity::CheckedTree
        }) {
            return Err(BuildEvaluationUsageError::MeasurementSponsorMismatch);
        }
        let record = BuildEvaluationUsage {
            measurement: evaluation_accounting::EvaluationMeasurementIdentity::CheckedTree,
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            invocation_work_budget: usage.work_budget(),
            sponsor_schema_version: sponsor.map(|sponsor| sponsor.limits().schema_version()),
            session_work_budget: sponsor.map(|sponsor| sponsor.limits().work_budget()),
            session_build_log_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_build_log_bytes()),
            session_filesystem_attempt_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_filesystem_operation_attempts()),
            session_live_filesystem_handle_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_live_filesystem_handles()),
            session_live_storage_cell_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_live_cells()),
            session_live_payload_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
            session_result_storage_cell_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_result_cells()),
            session_result_payload_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
            session_peak_live_filesystem_handles: sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
            session_peak_live_storage_cells: sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_cells),
            session_peak_live_payload_bytes: sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
            work_units: usage.work_units(),
            build_log_bytes: usage.build_log_bytes(),
            filesystem_operation_attempts: usage.filesystem_operation_attempts(),
            peak_live_storage_cells: usage.peak_live_cells(),
            peak_live_payload_bytes: usage.peak_live_text_bytes(),
            result_storage_cells: usage.result_cells(),
            result_payload_bytes: usage.result_text_bytes(),
        };
        record.validate()?;
        Ok(record)
    }
}

pub(crate) mod build_time {
    use crate::value::{Cell, Reference, Value};
    use build_time_values::BuildTimeValue;
    use std::collections::BTreeMap;

    pub(crate) fn materialize_build_time_value<E>(
        argument: BuildTimeValue,
        allocate: &impl Fn(Value) -> Result<Cell, E>,
        allocate_text: &impl Fn(Vec<u8>) -> Result<Value, E>,
    ) -> Result<Value, E> {
        match argument {
            BuildTimeValue::Unit => Ok(Value::Unit),
            BuildTimeValue::LayoutTarget(facts) => {
                let fields = [
                    ("profile", BuildTimeValue::Text(facts.profile.into_bytes())),
                    (
                        "pointer_size",
                        BuildTimeValue::Int(facts.pointer_size as i64),
                    ),
                    (
                        "pointer_alignment",
                        BuildTimeValue::Int(facts.pointer_alignment as i64),
                    ),
                    ("little_endian", BuildTimeValue::Bool(facts.little_endian)),
                ];
                materialize_build_time_value(
                    BuildTimeValue::Struct {
                        type_name: "@compiler-layout-target".to_owned(),
                        fields: fields
                            .into_iter()
                            .map(|(name, value)| (name.to_owned(), value))
                            .collect(),
                    },
                    allocate,
                    allocate_text,
                )
            }
            BuildTimeValue::Int(value) => Ok(Value::Int(value)),
            BuildTimeValue::Bool(value) => Ok(Value::Bool(value)),
            BuildTimeValue::Float(value) => Ok(Value::Float(value)),
            BuildTimeValue::UndeterminedNaN(format) => Ok(Value::UndeterminedNaN(format)),
            BuildTimeValue::Text(bytes) => allocate_text(bytes),
            BuildTimeValue::Struct { type_name, fields } => {
                let mut cells: BTreeMap<crate::value::ValueName, Cell> = BTreeMap::new();
                for (name, value) in fields {
                    let value = materialize_build_time_value(value, allocate, allocate_text)?;
                    cells.insert(name.into(), allocate(value)?);
                }
                Ok(Value::Struct {
                    type_symbol: symbols::SymbolHandle::invalid(),
                    type_name: type_name.into(),
                    fields: cells,
                })
            }
            BuildTimeValue::Case { variant, payload } => Ok(Value::Enum {
                type_symbol: symbols::SymbolHandle::invalid(),
                variant_name: variant.into(),
                payload: payload
                    .into_iter()
                    .map(|(name, value)| {
                        let value = materialize_build_time_value(value, allocate, allocate_text)?;
                        Ok((name.into(), allocate(value)?))
                    })
                    .collect::<Result<_, E>>()?,
            }),
            BuildTimeValue::Array(elements) => Ok(Value::Array(
                elements
                    .into_iter()
                    .map(|element| {
                        let value = materialize_build_time_value(element, allocate, allocate_text)?;
                        allocate(value)
                    })
                    .collect::<Result<_, E>>()?,
            )),
        }
    }

    pub(crate) fn observe_build_time_value<E>(
        value: &Value,
        read_reference: &impl Fn(&Reference) -> Result<Value, E>,
    ) -> Result<BuildTimeValue, E> {
        Ok(match value {
            Value::Unit => BuildTimeValue::Unit,
            Value::Int(value) => BuildTimeValue::Int(*value),
            Value::Bool(value) => BuildTimeValue::Bool(*value),
            Value::Float(value) => BuildTimeValue::Float(*value),
            Value::UndeterminedNaN(format) => BuildTimeValue::UndeterminedNaN(*format),
            Value::Str(bytes) => BuildTimeValue::Text(bytes.borrow().to_vec()),
            Value::Struct {
                type_name, fields, ..
            } => BuildTimeValue::Struct {
                type_name: type_name.to_string(),
                fields: fields
                    .iter()
                    .map(|(name, cell)| {
                        Ok((
                            name.to_string(),
                            observe_build_time_value(&cell.borrow(), read_reference)?,
                        ))
                    })
                    .collect::<Result<_, E>>()?,
            },
            Value::Enum {
                variant_name,
                payload,
                ..
            } => BuildTimeValue::Case {
                variant: variant_name.to_string(),
                payload: payload
                    .iter()
                    .map(|(name, cell)| {
                        Ok((
                            name.to_string(),
                            observe_build_time_value(&cell.borrow(), read_reference)?,
                        ))
                    })
                    .collect::<Result<_, E>>()?,
            },
            Value::Array(elements) => BuildTimeValue::Array(
                elements
                    .iter()
                    .map(|cell| observe_build_time_value(&cell.borrow(), read_reference))
                    .collect::<Result<_, E>>()?,
            ),
            Value::Ref(reference) => {
                observe_build_time_value(&read_reference(reference)?, read_reference)?
            }
        })
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
                typed_trees::expression::ExpressionNode::Binary(binary) => {
                    [binary.left, binary.right]
                }
                typed_trees::expression::ExpressionNode::Call(call) => {
                    let [left, right] = program.expression_table.expression_handles(call.arguments)
                    else {
                        return Err("selected build-time Float call has wrong arity".into());
                    };
                    [*left, *right]
                }
                _ => {
                    return Err(
                        "selected build-time Float expression has no binary operands".into(),
                    );
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
}

pub(crate) mod virtual_clock {
    use checked_trees::CheckedTrees;
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VirtualClockOperand {
        Milliseconds,
        Ignored,
        ExactInteger(i64),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VirtualClockResult {
        Unit,
        Integer(PrimitiveType),
    }

    impl VirtualClockResult {
        pub fn matches_type(self, checked: &CheckedTrees, result: TypeReferenceHandle) -> bool {
            if result.is_valid() && !checked.type_reference_table.contains_type_reference(result) {
                return false;
            }
            match self {
                Self::Unit => matches!(
                    checked.type_reference_table.type_reference(result),
                    TypeReferenceNode::Unit
                ),
                Self::Integer(primitive) => {
                    primitive.is_fixed_integer()
                        && matches!(
                            checked.type_reference_table.type_reference(result),
                            TypeReferenceNode::Named { .. }
                        )
                        && checked.primitive_type_reference(result) == Some(primitive)
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VirtualClockOperation {
        Wait {
            maximum_milliseconds: i64,
            result: VirtualClockResult,
        },
        Read {
            primitive: PrimitiveType,
            units_per_millisecond: i64,
        },
    }

    impl VirtualClockOperation {
        pub fn result(self) -> VirtualClockResult {
            match self {
                Self::Wait { result, .. } => result,
                Self::Read { primitive, .. } => VirtualClockResult::Integer(primitive),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct VirtualClockBinding {
        checked_program_address: usize,
        pub(crate) requirement: SymbolHandle,
        pub(crate) call_target: SymbolHandle,
        pub(crate) operands: Vec<VirtualClockOperand>,
        pub(crate) parameter_types: Vec<PrimitiveType>,
        pub(crate) operation: VirtualClockOperation,
        realization: SymbolHandle,
        realization_call_target: SymbolHandle,
    }

    impl VirtualClockBinding {
        pub fn from_compiler_resolved_requirement(
            checked: &CheckedTrees,
            requirement: SymbolHandle,
            operands: &[VirtualClockOperand],
            operation: VirtualClockOperation,
        ) -> Result<Self, &'static str> {
            let durations = operands
                .iter()
                .filter(|operand| **operand == VirtualClockOperand::Milliseconds)
                .count();
            match operation {
                VirtualClockOperation::Wait {
                    maximum_milliseconds,
                    ..
                } if maximum_milliseconds >= 0 && durations == 1 => {}
                VirtualClockOperation::Read {
                    units_per_millisecond,
                    primitive,
                    ..
                } if units_per_millisecond > 0
                    && durations == 0
                    && matches!(primitive, PrimitiveType::I64 | PrimitiveType::U64) => {}
                _ => {
                    return Err(
                        "virtual clock operation has an invalid duration or scale contract",
                    );
                }
            }
            let (parameters, return_type, call_target) =
                requirement_signature(checked, requirement)
                    .ok_or("virtual wait binding is not one exact bodyless requirement")?;
            if parameters.len() != operands.len()
                || !operation.result().matches_type(checked, return_type)
            {
                return Err(
                    "virtual wait binding requires its exact operand count and result type",
                );
            }
            let parameter_types = parameters
                .iter()
                .map(
                    |parameter| match checked.primitive_type_reference(parameter.type_reference) {
                        Some(primitive) if primitive.is_fixed_integer() => Ok(primitive),
                        _ => Err("virtual wait requires scalar integer operands"),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Self {
                checked_program_address: std::ptr::from_ref(checked).addr(),
                requirement,
                call_target,
                operands: operands.to_vec(),
                parameter_types,
                operation,
                realization: SymbolHandle::invalid(),
                realization_call_target: SymbolHandle::invalid(),
            })
        }

        pub fn with_compiler_resolved_realization(
            mut self,
            checked: &CheckedTrees,
            realization: SymbolHandle,
        ) -> Result<Self, &'static str> {
            if self.checked_program_address != std::ptr::from_ref(checked).addr() {
                return Err("virtual clock binding belongs to a different checked program");
            }
            let machine = checked
                .machine_by_symbol(realization)
                .filter(|machine| {
                    !machine.body_is_present && checked.machine_type_parameters(machine).is_empty()
                })
                .ok_or("virtual clock realization must be one exact bodyless machine")?;
            let [entry] = checked.machine_states(machine) else {
                return Err("virtual clock realization has no unique entry");
            };
            let parameters = checked.state_parameters(entry);
            if parameters.len() != self.parameter_types.len()
                || !parameters
                    .iter()
                    .zip(&self.parameter_types)
                    .all(|(parameter, primitive)| {
                        checked.primitive_type_reference(parameter.type_reference)
                            == Some(*primitive)
                    })
                || !self
                    .operation
                    .result()
                    .matches_type(checked, entry.return_type)
                || checked
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter(|conformance| conformance.requirement_symbol == self.requirement)
                    .count()
                    != 1
            {
                return Err("virtual clock realization disagrees with its exact requirement");
            }
            self.realization = realization;
            self.realization_call_target = entry.symbol;
            Ok(self)
        }

        pub(crate) fn matches_target(&self, target: SymbolHandle) -> bool {
            target == self.call_target
                || (self.realization_call_target.is_valid()
                    && target == self.realization_call_target)
        }

        pub(crate) fn overlaps(&self, other: &Self) -> bool {
            self.requirement == other.requirement
                || self.matches_target(other.call_target)
                || (other.realization_call_target.is_valid()
                    && self.matches_target(other.realization_call_target))
        }

        pub(crate) fn validate_for(&self, checked: &CheckedTrees) -> Result<(), &'static str> {
            if self.checked_program_address != std::ptr::from_ref(checked).addr() {
                return Err("virtual wait binding belongs to a different checked program");
            }
            let reconstructed = Self::from_compiler_resolved_requirement(
                checked,
                self.requirement,
                &self.operands,
                self.operation,
            )?;
            let reconstructed = if self.realization.is_valid() {
                reconstructed.with_compiler_resolved_realization(checked, self.realization)?
            } else {
                reconstructed
            };
            if reconstructed != *self {
                return Err("virtual wait binding no longer matches its checked requirement");
            }
            Ok(())
        }
    }

    fn requirement_signature(
        checked: &CheckedTrees,
        requirement: SymbolHandle,
    ) -> Option<(&[StateParameter], TypeReferenceHandle, SymbolHandle)> {
        if !requirement.is_valid() {
            return None;
        }
        let mut signatures = Vec::new();
        for machine in checked
            .machines()
            .iter()
            .filter(|machine| machine.symbol == requirement)
        {
            if machine.body_is_present
                || machine.supply_mode != MachineSupplyMode::TopLevelRequirement
            {
                return None;
            }
            let [entry] = checked.machine_states(machine) else {
                return None;
            };
            signatures.push((
                checked.state_parameters(entry),
                entry.return_type,
                entry.symbol,
            ));
        }
        for definition in checked
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
        {
            for signature in checked
                .trait_machine_signatures(definition)
                .iter()
                .filter(|signature| signature.symbol == requirement)
            {
                if !signature.native_callback_parameters.is_empty()
                    || !checked
                        .state_signature_type_parameters(signature)
                        .is_empty()
                {
                    return None;
                }
                signatures.push((
                    checked.state_signature_parameters(signature),
                    signature.return_type,
                    signature.symbol,
                ));
            }
        }
        match signatures.as_slice() {
            [signature] => Some(*signature),
            _ => None,
        }
    }
}
