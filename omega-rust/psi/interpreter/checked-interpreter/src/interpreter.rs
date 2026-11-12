/*
    Checked-tree execution entrypoints, scoped worker lifecycle, and the
    invocation-local `Evaluator` state machine.

    We configure authority and step budgets on a dedicated 256 MiB worker stack,
    bind the selected entry or build-time machine, and coordinate evaluation
    frames, virtual and physical filesystem state, and resource metering.
*/

pub(crate) mod evaluation_error {
    use std::fmt;

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
}

pub(crate) mod evaluator {
    pub(crate) use crate::build_machine;
    pub(crate) use crate::filesystem::evaluator_filesystem as filesystem;

    pub(crate) mod directory_entries {
        use super::{
            EvalResult, Halt, MAX_DIRECTORY_ENTRY_NAME_BYTES, MAX_DIRECTORY_SNAPSHOT_BYTES,
        };

        pub(crate) fn portable_directory_entry_name(name: &[u8]) -> &[u8] {
            &name[..name.len().min(MAX_DIRECTORY_ENTRY_NAME_BYTES)]
        }

        pub(crate) fn dirent_record_extent(name_len: usize) -> EvalResult<usize> {
            if name_len > MAX_DIRECTORY_ENTRY_NAME_BYTES {
                return Err(Halt::Trap(format!(
                    "untruncated directory entry name reached the dirent packer ({name_len} > {MAX_DIRECTORY_ENTRY_NAME_BYTES})"
                )));
            }
            let unaligned = 25usize.checked_add(name_len).ok_or_else(|| {
                Halt::Resource("directory entry record extent overflowed usize".to_owned())
            })?;
            let reclen = unaligned
                .checked_add(7)
                .map(|extent| extent / 8 * 8)
                .ok_or_else(|| {
                    Halt::Resource("directory entry record alignment overflowed usize".to_owned())
                })?;
            debug_assert!(reclen <= u16::MAX as usize);
            Ok(reclen)
        }

        pub(crate) fn checked_directory_name_snapshot_total(
            current: usize,
            name_len: usize,
        ) -> EvalResult<usize> {
            if name_len > MAX_DIRECTORY_ENTRY_NAME_BYTES {
                return Err(Halt::Trap(format!(
                    "untruncated directory entry name reached snapshot accounting ({name_len} > {MAX_DIRECTORY_ENTRY_NAME_BYTES})"
                )));
            }
            let total = current.checked_add(name_len).ok_or_else(|| {
                Halt::Resource(
                    "directory enumeration retained-name extent overflowed usize".to_owned(),
                )
            })?;
            if total > MAX_DIRECTORY_SNAPSHOT_BYTES {
                return Err(Halt::Resource(format!(
                    "directory enumeration retained names exceeded their {MAX_DIRECTORY_SNAPSHOT_BYTES}-byte logical payload ceiling"
                )));
            }
            Ok(total)
        }

        pub(crate) fn checked_directory_record_snapshot_total(
            current: usize,
            name_len: usize,
        ) -> EvalResult<usize> {
            let reclen = dirent_record_extent(name_len)?;
            let total = current.checked_add(reclen).ok_or_else(|| {
                Halt::Resource(
                    "directory enumeration packed-record extent overflowed usize".to_owned(),
                )
            })?;
            if total > MAX_DIRECTORY_SNAPSHOT_BYTES {
                return Err(Halt::Resource(format!(
                    "directory enumeration packed records exceeded their {MAX_DIRECTORY_SNAPSHOT_BYTES}-byte logical payload ceiling"
                )));
            }
            Ok(total)
        }

        pub(crate) fn pack_dirent_records(entries: &[(Vec<u8>, u8)]) -> EvalResult<Vec<u8>> {
            let total = entries.iter().try_fold(0usize, |total, (name, _)| {
                checked_directory_record_snapshot_total(
                    total,
                    portable_directory_entry_name(name).len(),
                )
            })?;
            let mut buffer = Vec::with_capacity(total);
            for (name, d_type) in entries {
                let name = portable_directory_entry_name(name);
                let namlen = name.len();
                let reclen = dirent_record_extent(namlen)?;
                let start = buffer.len();
                buffer.resize(start + reclen, 0);
                buffer[start + 16..start + 18].copy_from_slice(&(reclen as u16).to_le_bytes());
                buffer[start + 18..start + 20].copy_from_slice(&(namlen as u16).to_le_bytes());
                buffer[start + 20] = *d_type;
                buffer[start + 21..start + 21 + namlen].copy_from_slice(name);
            }
            debug_assert_eq!(buffer.len(), total);
            Ok(buffer)
        }

        pub(crate) fn dirent_record_chunk(
            records: &[u8],
            start: usize,
            count: usize,
        ) -> (&[u8], usize) {
            if start >= records.len() || count == 0 {
                return (&records[0..0], start);
            }

            let limit = start.saturating_add(count).min(records.len());
            let mut end = start;
            while end + 18 <= records.len() {
                let reclen = u16::from_le_bytes([records[end + 16], records[end + 17]]) as usize;
                if reclen == 0 || end + reclen > records.len() || end + reclen > limit {
                    break;
                }
                end += reclen;
            }
            (&records[start..end], end)
        }
    }

    pub(crate) mod halts {
        pub(crate) enum Halt {
            Exit(i32),
            Crash(typed_trees::signature::CrashCause, String),
            Unsupported(String),
            Trap(String),
            Resource(String),
            StaticInputUnavailable(crate::interpreter::UnavailableStaticInput),
        }

        pub(crate) type EvalResult<T> = Result<T, Halt>;

        pub(crate) fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
            Err(Halt::Unsupported(message.into()))
        }

        pub(crate) fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
            Err(Halt::Trap(message.into()))
        }
    }

    pub(crate) mod scalar_numerics {
        use super::{EvalResult, Evaluator, Halt, Value, trap};
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::bignum::BigInt;
        use numerics::float_projection::FloatProjectionOperation;
        use numerics::float_semantics::{
            FloatFormat as SemanticFloatFormat, FloatMeaning, FloatToIntegerError,
            IntegerFormat as SemanticIntegerFormat,
        };
        use typed_trees::types::PrimitiveType;

        impl Evaluator<'_> {
            pub(crate) fn semantic_float_result(
                &self,
                meaning: FloatMeaning,
                format: SemanticFloatFormat,
            ) -> Value {
                if self.semantic_float_evaluation && meaning.is_nan() {
                    Value::UndeterminedNaN(if format == SemanticFloatFormat::BINARY32 {
                        numerics::literals::FloatFormat::F32
                    } else {
                        numerics::literals::FloatFormat::F64
                    })
                } else {
                    Value::Float(meaning.to_interpreter_value(format))
                }
            }

            pub(crate) fn float_operand(
                &self,
                value: &Value,
                format: SemanticFloatFormat,
            ) -> EvalResult<FloatMeaning> {
                if let Value::UndeterminedNaN(landed) = value {
                    let expected = match landed {
                        numerics::literals::FloatFormat::F32 => SemanticFloatFormat::BINARY32,
                        numerics::literals::FloatFormat::F64 => SemanticFloatFormat::BINARY64,
                    };
                    if expected != format {
                        return trap("floating operand lost its landed format");
                    }
                    return Ok(FloatMeaning::NaN);
                }
                value
                    .as_float()
                    .map(|value| project_landed_float(format, value))
                    .ok_or_else(|| Halt::Trap("non-numeric float operand".to_owned()))
            }
        }

        pub(crate) fn project_landed_float(
            format: SemanticFloatFormat,
            value: f64,
        ) -> FloatMeaning {
            if format == SemanticFloatFormat::BINARY32 {
                FloatProjectionOperation::Meaning32
                    .project_f32(value as f32)
                    .expect("binary32 projection row accepts f32")
            } else {
                FloatProjectionOperation::Meaning64
                    .project_f64(value)
                    .expect("binary64 projection row accepts f64")
            }
        }

        pub(crate) fn integer_bounds(ty: PrimitiveType) -> Option<(i64, i64)> {
            match ty {
                PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
                PrimitiveType::U8 => Some((0, u8::MAX as i64)),
                PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
                PrimitiveType::U16 => Some((0, u16::MAX as i64)),
                PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
                PrimitiveType::U32 => Some((0, u32::MAX as i64)),
                PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
                _ => None,
            }
        }

        pub(crate) fn semantic_integer_format(ty: PrimitiveType) -> Option<SemanticIntegerFormat> {
            match ty {
                PrimitiveType::I8 => Some(SemanticIntegerFormat::I8),
                PrimitiveType::I16 => Some(SemanticIntegerFormat::I16),
                PrimitiveType::I32 => Some(SemanticIntegerFormat::I32),
                PrimitiveType::I64 => Some(SemanticIntegerFormat::I64),
                PrimitiveType::U8 => Some(SemanticIntegerFormat::U8),
                PrimitiveType::U16 => Some(SemanticIntegerFormat::U16),
                PrimitiveType::U32 => Some(SemanticIntegerFormat::U32),
                PrimitiveType::U64 | PrimitiveType::Addr => Some(SemanticIntegerFormat::U64),
                PrimitiveType::Selector
                | PrimitiveType::Bool
                | PrimitiveType::F32
                | PrimitiveType::F64 => None,
            }
        }

        pub(crate) fn is_unsigned_integer_primitive(ty: PrimitiveType) -> bool {
            matches!(
                ty,
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
            )
        }

        pub(crate) fn big_integer_runtime_value(value: &BigInt, ty: PrimitiveType) -> i64 {
            if is_unsigned_integer_primitive(ty) {
                value
                    .to_u64()
                    .expect("checked unsigned conversion fits its target") as i64
            } else {
                value
                    .to_i64()
                    .expect("checked signed conversion fits its target")
            }
        }

        pub(crate) fn float_to_integer_trap_message(
            target: PrimitiveType,
            reason: FloatToIntegerError,
            trapping: bool,
        ) -> String {
            let operation = if trapping { "Trapping" } else { "Exact" };
            let reason = match reason {
                FloatToIntegerError::NonFinite => "the value is not finite",
                FloatToIntegerError::OutOfRange => "the truncated value is out of range",
            };
            format!("float-to-int conversion failed in {operation} domain: {reason} for {target:?}")
        }

        pub(crate) fn apply_arithmetic_domain(
            raw: i64,
            ty: PrimitiveType,
            domain: ArithmeticDomain,
        ) -> EvalResult<i64> {
            match domain {
                ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => Ok(wrap_to_width(raw, ty)),
                ArithmeticDomain::Saturating => match integer_bounds(ty) {
                    Some((min, max)) => Ok(raw.clamp(min, max)),
                    None => Ok(wrap_to_width(raw, ty)),
                },
                ArithmeticDomain::Trapping => match integer_bounds(ty) {
                    Some((min, max)) if raw < min || raw > max => trap(format!(
                        "arithmetic overflow in Trapping domain: {raw} is out of range for {ty:?}"
                    )),
                    _ => Ok(wrap_to_width(raw, ty)),
                },
            }
        }

        pub(crate) fn primitive_bit_width(ty: PrimitiveType) -> u64 {
            match ty {
                PrimitiveType::I8 | PrimitiveType::U8 => 8,
                PrimitiveType::I16 | PrimitiveType::U16 => 16,
                PrimitiveType::I32 | PrimitiveType::U32 => 32,
                _ => 64,
            }
        }

        pub(crate) fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
            match ty {
                PrimitiveType::I8 => raw as i8 as i64,
                PrimitiveType::U8 => raw as u8 as i64,
                PrimitiveType::I16 => raw as i16 as i64,
                PrimitiveType::U16 => raw as u16 as i64,
                PrimitiveType::I32 => raw as i32 as i64,
                PrimitiveType::U32 => raw as u32 as i64,
                PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => raw,
                PrimitiveType::Selector
                | PrimitiveType::Bool
                | PrimitiveType::F32
                | PrimitiveType::F64 => raw,
            }
        }

        pub(crate) fn integer_primitive_byte_width(ty: PrimitiveType) -> Option<usize> {
            match ty {
                PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
                PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
                PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
                PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => Some(8),
                PrimitiveType::Selector
                | PrimitiveType::Bool
                | PrimitiveType::F32
                | PrimitiveType::F64 => None,
            }
        }

        pub(crate) fn primitive_is_unsigned64(primitive: Option<PrimitiveType>) -> bool {
            matches!(primitive, Some(PrimitiveType::U64 | PrimitiveType::Addr))
        }
    }

    pub(crate) use crate::build_evaluation_sponsor::BuildEvaluationLiveFilesystemHandleLease;
    pub(crate) use crate::value::{Cell, CellMeter, TextByteMeter, Value};
    pub(crate) use crate::value::{Reference, ReferenceProjection, ReferenceStorage};
    pub(crate) use crate::{
        BuildEvaluationSponsor, EvaluationUsage, FilesystemEvaluationHaltKind,
        FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
        FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
        FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
        FilesystemMetadataLayout, FilesystemObservationProvider, FilesystemOperationAttempt,
        FilesystemOperationAttemptOutcome, FilesystemOperationResult,
        PrivateLayoutPlacementReceipt,
    };
    pub(crate) use checked_trees::{CheckedOperatorFacts, CheckedTrees};
    pub(crate) use language_semantics::declaration_selection::{
        DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
    };
    pub(crate) use numerics::arithmetic::ArithmeticDomain;
    pub(crate) use numerics::bignum::BigInt;
    pub(crate) use numerics::float_semantics::{
        FloatClass as SemanticFloatClass, FloatFormat as SemanticFloatFormat, FloatMeaning,
        FloatPolicyTrap, FloatSemantics,
    };
    pub(crate) use semantic_vocabulary::ProgramEntrySourceCatalog;
    pub(crate) use std::collections::{BTreeMap, HashSet};
    pub(crate) use symbols::SymbolHandle;
    pub(crate) use typed_trees::data::{DataDefinition, DataMember};
    pub(crate) use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableNamePath,
        UnaryOperator,
    };
    pub(crate) use typed_trees::machine::Machine;
    pub(crate) use typed_trees::state::State;
    pub(crate) use typed_trees::statement::{
        StatementNode, StatementTable, TableCall, TableTransition, TransitionGuardNode,
        TransitionTargetNode,
    };
    pub(crate) use typed_trees::types::{
        FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable,
    };
    pub(crate) use typed_trees::{TypedTreeTables, TypedTrees};

    pub(crate) use crate::interpreter::evaluator::filesystem::logical_handle_store::FilesystemLogicalHandles;
    pub(crate) use build_machine::output_obligations::OutputSealState;
    pub(crate) use build_machine::paths::{rooted_build_path_parts, validate_build_relative_path};
    pub(crate) use directory_entries::{
        checked_directory_name_snapshot_total, checked_directory_record_snapshot_total,
        dirent_record_chunk, pack_dirent_records, portable_directory_entry_name,
    };
    pub(crate) use filesystem::host_operation::{
        FilesystemHostOperation, FilesystemHostResultKind,
    };
    pub(crate) use filesystem::preparation::{
        FIND_DATA_OUTPUT_BYTES, PreparedByteOutput, PreparedFilesystemCall,
        PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
        STAT_OUTPUT_BYTES, synthetic_handle_fd,
    };
    pub(crate) use halts::Halt;
    pub(crate) use halts::{EvalResult, trap, unsupported};
    pub(crate) use numerics::float_representation::{
        binary32_from_bits as interpreter_f32_from_bits,
        binary32_to_bits as interpreter_f32_to_bits,
    };
    pub(crate) use scalar_numerics::{
        apply_arithmetic_domain, big_integer_runtime_value, float_to_integer_trap_message,
        integer_bounds, integer_primitive_byte_width, is_unsigned_integer_primitive,
        primitive_bit_width, primitive_is_unsigned64, semantic_integer_format, wrap_to_width,
    };
    pub(crate) use typed_trees::expression::CompletedValueCapture;

    pub(crate) const STEP_BUDGET: u64 = 10_000_000;

    pub(crate) fn ambient_step_budget() -> u64 {
        std::env::var("OMEGA_INTERP_STEP_BUDGET")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(STEP_BUDGET)
    }

    pub(crate) const CONST_EVAL_STEP_BUDGET: u64 = 100_000;
    pub(crate) const CALL_DEPTH_BUDGET: u32 = 512;
    pub(crate) const MAX_DIRECTORY_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
    pub(crate) const MAX_DIRECTORY_ENTRY_NAME_BYTES: usize = 255;

    pub(crate) const VIRTUAL_MTIME_SECS: i64 = 1_000_000_000;
    pub(crate) const VIRTUAL_UID: u32 = 501;
    pub(crate) const VIRTUAL_GID: u32 = 20;

    #[derive(Clone)]
    pub(crate) enum ReferenceProjectionStep {
        Field(String),
        Index(ExpressionHandle),
    }

    pub(crate) struct MutableRecordRecastWrite {
        pub(crate) recast: ReferenceStorage,
        pub(crate) projection: ReferenceProjection,
    }

    pub(crate) struct FrameLocal {
        pub(crate) symbol: SymbolHandle,
        pub(crate) cell: Cell,
        pub(crate) type_reference: TypeReferenceHandle,
        pub(crate) scalar: Option<(PrimitiveType, ArithmeticDomain)>,
    }

    pub(crate) struct Frame {
        pub(crate) return_type: TypeReferenceHandle,
        pub(crate) locals: Vec<FrameLocal>,
        pub(crate) machine_symbol: SymbolHandle,
        pub(crate) state_symbol: SymbolHandle,
        pub(crate) guard_call_results: Vec<(ExpressionHandle, Value)>,
        pub(crate) scalar_expression_depth: usize,
        pub(crate) scalar_expression_values: Vec<(ExpressionHandle, Value)>,
        pub(crate) match_subjects: Vec<(arena::Handle<CompletedValueCapture>, Cell)>,
    }

    pub(crate) struct VirtualFd {
        pub(crate) path: Vec<u8>,
        pub(crate) cursor: std::rc::Rc<std::cell::Cell<usize>>,
        pub(crate) writable: bool,
        pub(crate) is_dir: bool,
    }

    pub(crate) struct Evaluator<'program> {
        pub(crate) program: &'program TypedTrees,
        pub(crate) product_declarations: &'program TypedTrees,
        pub(crate) checked: Option<&'program CheckedTrees>,
        pub(crate) operator_facts: Option<&'program CheckedOperatorFacts>,
        pub(crate) selected_build_time_operators:
            &'program [crate::SelectedBuildTimeBinaryOperator],
        pub(crate) selected_build_time_machine_bodies:
            &'program [crate::SelectedBuildTimeMachineBody],
        pub(crate) semantic_float_evaluation: bool,
        pub(crate) stdout: Vec<u8>,
        pub(crate) stderr: Vec<u8>,
        pub(crate) build_log: Vec<u8>,
        pub(crate) root_build: Option<Cell>,
        pub(crate) executed_source_selections:
            Vec<language_semantics::declaration_selection::ExecutedBuildSelection>,
        pub(crate) pending_build_selections:
            Vec<arena::Handle<typed_trees::expression::BuildSelectionRequest>>,
        pub(crate) executed_behavior_exclusions: Vec<crate::ExecutedBehaviorExclusion>,
        pub(crate) executed_build_selections: Vec<crate::ExecutedBuildSelection>,
        pub(crate) executed_wire_demands: Vec<typed_trees::expression::ExpressionHandle>,
        pub(crate) product_entry_descriptions: Vec<DescribedProductEntry>,
        pub(crate) product_entry_slots: Option<&'program dyn ProgramEntrySourceCatalog>,
        pub(crate) product_schema_descriptions: Vec<DescribedProductSchema>,
        pub(crate) product_provider_descriptions: Vec<DescribedProductProvider>,
        pub(crate) stdin: &'program [u8],
        pub(crate) stdin_cursor: usize,
        pub(crate) virtual_ticks: i64,
        pub(crate) virtual_live_windows: std::collections::HashSet<i64>,
        pub(crate) virtual_window_next: i64,
        pub(crate) virtual_files: BTreeMap<Vec<u8>, Vec<u8>>,
        pub(crate) virtual_fds: BTreeMap<i32, VirtualFd>,
        pub(crate) virtual_next_fd: i32,
        pub(crate) virtual_dirs: std::collections::BTreeSet<Vec<u8>>,
        pub(crate) filesystem_metadata_layout: FilesystemMetadataLayout,
        pub(crate) virtual_finds: BTreeMap<i64, std::collections::VecDeque<(Vec<u8>, bool)>>,
        pub(crate) virtual_next_find: i64,
        pub(crate) virtual_perms: BTreeMap<Vec<u8>, u32>,
        pub(crate) virtual_symlinks: BTreeMap<Vec<u8>, Vec<u8>>,
        pub(crate) virtual_times: BTreeMap<Vec<u8>, i64>,
        pub(crate) virtual_flocks: BTreeMap<Vec<u8>, i32>,
        pub(crate) virtual_char_devices: std::collections::BTreeSet<Vec<u8>>,
        pub(crate) virtual_errno: i32,
        pub(crate) rooted_build_paths_required: bool,
        pub(crate) output_obligations: Vec<crate::BuildOutputObligation>,
        pub(crate) output_receipts: Vec<crate::BuildOutputReceipt>,
        pub(crate) output_obligation_paths: BTreeMap<(FilesystemGrantRootIdentity, Vec<u8>), usize>,
        pub(crate) output_seal_states:
            BTreeMap<(FilesystemGrantRootIdentity, Vec<u8>), OutputSealState>,
        pub(crate) output_open_writers:
            BTreeMap<FilesystemLogicalHandleIdentity, (FilesystemGrantRootIdentity, Vec<u8>)>,
        pub(crate) host_boundary_touched: bool,
        pub(crate) filesystem_operation_attempts: Vec<FilesystemOperationAttempt>,
        pub(crate) filesystem_logical_handles: FilesystemLogicalHandles,
        pub(crate) filesystem_live_handle_leases: BTreeMap<
            crate::FilesystemLogicalHandleIdentity,
            BuildEvaluationLiveFilesystemHandleLease,
        >,
        pub(crate) filesystem_observation_resource_halt: Option<String>,
        pub(crate) filesystem_operation_attempt_stack: Vec<usize>,
        pub(crate) private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
        pub(crate) usage: EvaluationUsage,
        pub(crate) cell_meter: CellMeter,
        pub(crate) text_byte_meter: TextByteMeter,
        pub(crate) build_evaluation_sponsor: Option<BuildEvaluationSponsor>,
        pub(crate) step_budget: u64,
        pub(crate) call_depth: u32,
        pub(crate) guard_depth: u32,
    }

    #[derive(Clone)]
    pub(crate) struct EvaluatedArgument {
        pub(crate) cell: Cell,
    }

    impl EvaluatedArgument {
        pub(crate) fn plain(cell: Cell) -> Self {
            Self { cell }
        }
    }

    pub(crate) enum TransitionDecision<'program> {
        Terminal,
        Value(Value),
        Named {
            state: &'program State,
            machine: &'program Machine,
            arguments: Vec<EvaluatedArgument>,
        },
    }

    impl Frame {
        pub(crate) fn local(&self, symbol: SymbolHandle) -> Option<&FrameLocal> {
            if !symbol.is_valid() {
                return None;
            }
            self.locals.iter().find(|local| local.symbol == symbol)
        }

        pub(crate) fn local_cell(&self, symbol: SymbolHandle) -> Option<Cell> {
            self.local(symbol).map(|local| local.cell.clone())
        }

        pub(crate) fn local_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
            self.local(symbol).map(|local| local.type_reference)
        }

        pub(crate) fn local_scalar(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(PrimitiveType, ArithmeticDomain)> {
            self.local(symbol).and_then(|local| local.scalar)
        }

        pub(crate) fn bind(&mut self, local: FrameLocal) {
            match self
                .locals
                .iter_mut()
                .find(|existing| existing.symbol == local.symbol)
            {
                Some(existing) => *existing = local,
                None => self.locals.push(local),
            }
        }

        pub(crate) fn guard_call_result(
            &self,
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<Value> {
            self.guard_call_results
                .iter()
                .find(|(seen, _)| {
                    expression_table(program).expressions_structurally_equal(*seen, expression)
                })
                .map(|(_, value)| value.clone())
        }

        pub(crate) fn record_guard_call_result(
            &mut self,
            expression: ExpressionHandle,
            value: &Value,
        ) {
            self.guard_call_results.push((expression, value.clone()));
        }
    }

    pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
        &tables.expression_table
    }

    pub(crate) fn type_reference_table(tables: &TypedTreeTables) -> &TypeReferenceTable {
        &tables.type_reference_table
    }

    pub(crate) fn statement_table(tables: &TypedTreeTables) -> &StatementTable {
        &tables.statement_table
    }
}

use crate::{BuildTimeOperationEvaluation, SelectedBuildTimeBinaryOperator};
use build_time_values::BuildTimeValue;
pub use evaluation_error::{BuildMachineEvaluationError, UnavailableStaticInput};
use evaluator::{CONST_EVAL_STEP_BUDGET, Evaluator, Halt};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

const INTERPRETER_STACK_SIZE: usize = 256 * 1024 * 1024;

thread_local! {
    static ON_INTERPRETER_STACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn run_with_interpreter_stack<Output: Send>(
    work: impl FnOnce() -> Output + Send,
) -> std::thread::Result<Output> {
    if ON_INTERPRETER_STACK.get() {
        return std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(std::env::var("OMEGA_PROBE_STACK_MB_INTERP").ok().and_then(|v| v.parse::<usize>().ok()).map(|mb| mb * 1024 * 1024).unwrap_or(INTERPRETER_STACK_SIZE))
            .spawn_scoped(scope, || {
                ON_INTERPRETER_STACK.set(true);
                work()
            })
            .expect("spawn interpreter worker thread")
            .join()
    })
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

#[derive(Clone, Copy)]
pub enum BuildMachineProgram<'program> {
    Typed(&'program TypedTrees),
    Checked(&'program checked_trees::CheckedTrees),
    CheckedClosure {
        execution: &'program checked_trees::CheckedTrees,
        product_declarations: &'program TypedTrees,
    },
}

impl<'program> BuildMachineProgram<'program> {
    fn typed(self) -> &'program TypedTrees {
        match self {
            Self::Typed(typed) => typed,
            Self::Checked(checked) => &checked.typed,
            Self::CheckedClosure { execution, .. } => &execution.typed,
        }
    }
}

impl<'a> BuildMachineEvaluationRequest<'a> {
    pub fn named(machine_name: &'a str, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Name(machine_name),
            arguments,
            operators: &[],
            machine_bodies: &[],
        }
    }

    pub fn symbol(machine_symbol: SymbolHandle, arguments: Vec<BuildTimeValue>) -> Self {
        Self {
            entry: BuildMachineEntry::Symbol(machine_symbol),
            arguments,
            operators: &[],
            machine_bodies: &[],
        }
    }
}

pub fn materialize_zero_initialized_build_time_value(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<BuildTimeValue, String> {
    let mut evaluator = Evaluator::new(program, &[]);
    let value =
        evaluator
            .default_value_for_type(type_reference)
            .map_err(|reason| match reason {
                Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => {
                    message
                }
                Halt::Exit(code) => format!("zero storage initialization exited with code {code}"),
                Halt::Crash(cause, site) => format!("explicit crash {cause:?} at {site}"),
                Halt::StaticInputUnavailable(input) => input.to_string(),
            })?;
    crate::build_time::observe_build_time_value(&value, &|reference| {
        evaluator.read_reference(reference)
    })
    .map_err(|reason| match reason {
        Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => message,
        Halt::Exit(code) => format!("zero storage observation exited with code {code}"),
        Halt::Crash(cause, site) => format!("explicit crash {cause:?} at {site}"),
        Halt::StaticInputUnavailable(input) => input.to_string(),
    })
}

pub const fn pure_evaluation_step_ceiling() -> u64 {
    CONST_EVAL_STEP_BUDGET
}

pub fn evaluate_prepared_build_time_machine(
    program: BuildMachineProgram<'_>,
    request: BuildMachineEvaluationRequest<'_>,
) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, BuildMachineEvaluationError> {
    evaluate_structured_return(
        program,
        request.entry,
        request.arguments,
        request.operators,
        request.machine_bodies,
    )
}

fn structured_return_halt_error(halt: Halt) -> BuildMachineEvaluationError {
    match halt {
        Halt::Exit(code) => BuildMachineEvaluationError::Rejected(format!(
            "the machine attempted to exit the process (code {code}) instead of returning a value"
        )),
        Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => {
            BuildMachineEvaluationError::Rejected(message)
        }
        Halt::Crash(cause, site) => {
            BuildMachineEvaluationError::Rejected(format!("explicit crash {cause:?} at {site}"))
        }
        Halt::StaticInputUnavailable(input) => {
            BuildMachineEvaluationError::StaticInputUnavailable(input)
        }
    }
}

fn evaluate_structured_return(
    program: BuildMachineProgram<'_>,
    entry: BuildMachineEntry<'_>,
    arguments: Vec<build_time_values::BuildTimeValue>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    machine_bodies: &[SelectedBuildTimeMachineBody],
) -> Result<
    BuildTimeOperationEvaluation<build_time_values::BuildTimeValue>,
    BuildMachineEvaluationError,
> {
    let typed = program.typed();
    SelectedBuildTimeMachineBody::validate_all(typed, machine_bodies)?;
    if operators
        .iter()
        .any(|operator| operator.has_crash_contract(typed))
    {
        return Err(BuildMachineEvaluationError::Rejected(
            "selected operator crash invocations have no build-time execution support".into(),
        ));
    }
    run_with_interpreter_stack(|| {
        let mut evaluator = Evaluator::new_build(program);
        evaluator.selected_build_time_operators = operators;
        evaluator.selected_build_time_machine_bodies = machine_bodies;
        evaluator.configure_build_evaluation(CONST_EVAL_STEP_BUDGET, None);
        let result = match entry {
            BuildMachineEntry::Name(machine_name) => {
                evaluator.run_build_time_machine(machine_name, arguments)
            }
            BuildMachineEntry::Symbol(machine_symbol) => {
                evaluator.run_build_time_machine_symbol(machine_symbol, arguments)
            }
        };
        evaluator.finish_cell_usage();
        let mut usage = evaluator.usage;
        match result {
            Ok(value) => {
                let (result_cells, result_text_bytes) =
                    build_time_result_custody(std::slice::from_ref(&value)).ok_or_else(|| {
                        "build-time evaluator result-cell count overflowed".to_owned()
                    })?;
                usage.record_result_custody(result_cells, result_text_bytes);
                for receipt in &evaluator.private_layout_placements {
                    let (cells, text_bytes) =
                        build_time_result_custody(std::slice::from_ref(&receipt.placement))
                            .ok_or_else(|| {
                                "private placement result-cell count overflowed".to_owned()
                            })?;
                    usage.record_result_custody(cells, text_bytes);
                }
                Ok(BuildTimeOperationEvaluation::new(
                    value,
                    usage,
                    evaluator.private_layout_placements,
                ))
            }
            Err(halt) => Err(structured_return_halt_error(halt)),
        }
    })
    .unwrap_or_else(|_| {
        Err(BuildMachineEvaluationError::Rejected(
            "build-time evaluator thread panicked".to_owned(),
        ))
    })
}

fn build_time_result_custody(values: &[build_time_values::BuildTimeValue]) -> Option<(u64, u64)> {
    values
        .iter()
        .try_fold((0u64, 0u64), |(cells, text_bytes), value| {
            Some((
                cells.checked_add(value.retained_cell_count()?)?,
                text_bytes.checked_add(value.retained_text_byte_count()?)?,
            ))
        })
}
