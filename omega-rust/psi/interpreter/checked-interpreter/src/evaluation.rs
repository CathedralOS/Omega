/*
    Measured checked-tree evaluation outcomes, build-time value observation,
    sponsor accounting, and virtual clock bindings.

    We record step schedules, storage cell and text-byte watermarks, output
    obligations, and executed behavior exclusions alongside the pure build-time
    value and virtual wait vocabulary.
*/

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
