//! Executed Build path operations selected by exact toolchain declaration.
//!
//! Declaration qualification supplies no runtime receiver. Both call forms
//! pass explicit operands to this owner; the first operand carries the private
//! activation facet. Filesystem operand evaluation stays inside its existing
//! accounting owner, so nested calls retain call-start order and live buffers.
use crate::interpreter::evaluator::{
    BTreeMap, Cell, EvalResult, Evaluator, ExpressionHandle, FilesystemHostOperation, Frame, Halt,
    SymbolHandle, Value,
};
use crate::{
    FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, FilesystemGrantRootIdentity,
    MAX_INCLUDED_BUILD_SOURCES,
};

pub(in crate::interpreter::evaluator) const ROOTED_BUILD_PATH_TYPE: &str = "$OmegaBuildRootedPath";
const SOURCE_ROOT_FACET_TYPE: &str = "$OmegaBuildSourceRoot";
pub(in crate::interpreter::evaluator) const OUTPUT_ROOT_FACET_TYPE: &str = "$OmegaBuildOutputRoot";
const INPUT_SLOTS_FACET_TYPE: &str = "$OmegaBuildInputSlots";

enum BuildPathOperation {
    Resolve(&'static str),
    NamedInput,
    Filesystem {
        facet: &'static str,
        operation: FilesystemHostOperation,
    },
    IncludeSource,
}

impl<'program> Evaluator<'program> {
    /// Name the exact toolchain Build facet method a resolved call target
    /// selects, as the `(attached data, state)` pair its prelude declaration
    /// carries. Every Build facet family keys on this one answer, so an
    /// ordinary user machine spelled `write` or `require` can never acquire
    /// Build host semantics, and a facet stays reachable through the explicit
    /// activation-operand spelling rather than through a receiver place.
    pub(in crate::interpreter::evaluator) fn build_prelude_facet_method(
        &self,
        target: SymbolHandle,
    ) -> Option<(&'program str, &'program str)> {
        if !target.is_valid() || !self.symbol_has_build_prelude_source(target) {
            return None;
        }
        let (machine, state) = if let Some(machine) = self.program.machine_by_symbol(target) {
            (machine, self.machine_entry_state(machine)?)
        } else {
            let machine = self.program.machine_holding_state(target)?;
            (machine, self.find_state_by_symbol(machine, target)?)
        };
        if !self.symbol_has_build_prelude_source(machine.symbol)
            || !self.symbol_has_build_prelude_source(state.symbol)
        {
            return None;
        }
        Some((
            machine.attached_data.as_ref()?.as_str(),
            state.name.as_str(),
        ))
    }

    /// Select only the resolved compiler-owned declaration. Ordinary user
    /// machines with the same spelling never acquire Build host semantics.
    fn build_path_operation(&self, target: SymbolHandle) -> Option<BuildPathOperation> {
        match self.build_prelude_facet_method(target)? {
            ("BuildSource", "resolve") => Some(BuildPathOperation::Resolve(SOURCE_ROOT_FACET_TYPE)),
            ("BuildOutput", "resolve") => Some(BuildPathOperation::Resolve(OUTPUT_ROOT_FACET_TYPE)),
            ("BuildInputs", "get") => Some(BuildPathOperation::NamedInput),
            ("BuildOutput", "include_source") => Some(BuildPathOperation::IncludeSource),
            ("BuildSource", method) => {
                let operation = match method {
                    "open" => FilesystemHostOperation::Open,
                    "read" => FilesystemHostOperation::Read,
                    "close" => FilesystemHostOperation::Close,
                    _ => return None,
                };
                Some(BuildPathOperation::Filesystem {
                    facet: SOURCE_ROOT_FACET_TYPE,
                    operation,
                })
            }
            ("BuildOutput", method) => {
                let operation = match method {
                    "create" => FilesystemHostOperation::Create,
                    "write" => FilesystemHostOperation::Write,
                    "close" => FilesystemHostOperation::Close,
                    _ => return None,
                };
                Some(BuildPathOperation::Filesystem {
                    facet: OUTPUT_ROOT_FACET_TYPE,
                    operation,
                })
            }
            _ => None,
        }
    }

    pub(in crate::interpreter::evaluator) fn try_build_path_call(
        &mut self,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(operation) = self.build_path_operation(target) else {
            return Ok(None);
        };
        let Some((facet, operands)) = arguments.split_first() else {
            return Err(Halt::Trap(
                "Build path operation has no explicit activation facet".into(),
            ));
        };
        let value = match operation {
            BuildPathOperation::Resolve(expected) => {
                let root = self.build_root_argument(*facet, expected, frame)?;
                let [relative] = operands else {
                    return Err(Halt::Trap(
                        "build-root resolution requires one relative path".into(),
                    ));
                };
                let relative = self.build_relative_bytes(*relative, frame)?;
                validate_build_relative_path(&relative)?;
                Value::Struct {
                    type_symbol: SymbolHandle::invalid(),
                    type_name: ROOTED_BUILD_PATH_TYPE.into(),
                    fields: BTreeMap::from([
                        (
                            "root".into(),
                            self.allocate_cell(Value::Int(i64::from(root.get())))?,
                        ),
                        (
                            "relative".into(),
                            self.allocate_cell(self.allocate_text(relative)?)?,
                        ),
                    ]),
                }
            }
            BuildPathOperation::NamedInput => {
                let facet = self.eval_argument(*facet, frame)?;
                let facet = self.deref_cell(facet);
                self.build_named_input(&facet, operands, frame)?
            }
            BuildPathOperation::Filesystem {
                facet: expected,
                operation,
            } => self.evaluate_filesystem_operation(operation, |evaluator| {
                evaluator.build_root_argument(*facet, expected, frame)?;
                evaluator.prepare_filesystem_call(operation, operands, frame)
            })?,
            BuildPathOperation::IncludeSource => {
                let root = self.build_root_argument(*facet, OUTPUT_ROOT_FACET_TYPE, frame)?;
                self.include_build_source(root, operands, frame)?
            }
        };
        Ok(Some(value))
    }

    /// Admit one explicit activation-facet operand and answer its grant-root
    /// identity. Every Build facet family takes its authority this way: an
    /// authored `BuildOutput {}` has the declared type name rather than the
    /// private activation marker, so it carries no root.
    pub(in crate::interpreter::evaluator) fn build_root_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: &str,
        frame: &mut Frame,
    ) -> EvalResult<FilesystemGrantRootIdentity> {
        let facet = self.eval_argument(expression, frame)?;
        let facet = self.deref_cell(facet);
        build_root_facet_identity(&facet.borrow(), expected)
    }

    /// Slot lookup returns an existing compiler-issued read root. A slot name
    /// is never interpreted as a pathname and cannot extend filesystem grants.
    fn build_named_input(
        &mut self,
        facet: &Cell,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let [name] = arguments else {
            return Err(Halt::Trap(
                "named build input lookup requires one slot name".into(),
            ));
        };
        // Validate the private facet before evaluating the selected name.
        if !matches!(&*facet.borrow(), Value::Struct { type_name, .. } if &**type_name == INPUT_SLOTS_FACET_TYPE)
        {
            return Err(Halt::Trap(
                "named build inputs require a compiler-issued facet".into(),
            ));
        }
        let name = self.build_relative_bytes(*name, frame)?;
        validate_build_relative_path(&name)?;
        let name = std::str::from_utf8(&name)
            .map_err(|_| Halt::Trap("named build input slot is not UTF-8".into()))?;
        let facet = facet.borrow();
        let Value::Struct { fields, .. } = &*facet else {
            return Err(Halt::Trap(
                "named build inputs lost their compiler-issued facet".into(),
            ));
        };
        let input = fields.get(name).ok_or_else(|| {
            Halt::Trap(format!(
                "named build input `{name}` is not assigned to this activation"
            ))
        })?;
        let value = input.borrow().clone();
        build_root_facet_identity(&value, SOURCE_ROOT_FACET_TYPE)?;
        Ok(value)
    }

    fn build_relative_bytes(
        &mut self,
        expression: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Vec<u8>> {
        match self.eval_expression(expression, frame)? {
            Value::Str(bytes) => Ok(bytes.borrow().to_vec()),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    cell.borrow()
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| {
                            Halt::Trap("build-root path contains a non-byte element".to_owned())
                        })
                })
                .collect(),
            other => Err(Halt::Trap(format!(
                "build-root path must be byte data, got {other:?}"
            ))),
        }
    }

    pub(in crate::interpreter::evaluator) fn enable_rooted_build_paths_from_arguments(
        &mut self,
        arguments: &[build_time_values::BuildTimeValue],
    ) {
        self.rooted_build_paths_required = arguments.iter().any(|argument| {
            let build_time_values::BuildTimeValue::Struct { fields, .. } = argument else {
                return false;
            };
            fields.iter().any(|(_, value)| {
                matches!(
                    value,
                    build_time_values::BuildTimeValue::Struct { type_name, .. }
                        if matches!(type_name.as_str(), SOURCE_ROOT_FACET_TYPE | OUTPUT_ROOT_FACET_TYPE)
                )
            })
        });
    }

    fn include_build_source(
        &mut self,
        output_root: FilesystemGrantRootIdentity,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let [included] = arguments else {
            return Err(Halt::Trap(
                "generated-source handoff requires one Output-rooted path".to_owned(),
            ));
        };
        let included = self.eval_expression(*included, frame)?;
        let Some((included_root, relative_path)) = rooted_build_path_parts(&included)? else {
            return Err(Halt::Trap(
                "generated-source handoff requires an Output-rooted path".to_owned(),
            ));
        };
        if included_root != output_root {
            return Err(Halt::Trap(
                "generated-source handoff path belongs to a different build root".to_owned(),
            ));
        }
        let scoped_real_output = self
            .real_fs
            .as_ref()
            .is_some_and(crate::interpreter::evaluator::filesystem::real::RealFs::is_scoped);
        if !scoped_real_output {
            return Err(Halt::Trap(
                "generated-source handoff requires a scoped build-output grant".to_owned(),
            ));
        }
        if self.build_included_sources.len() == MAX_INCLUDED_BUILD_SOURCES {
            return Err(Halt::Resource(format!(
                "generated-source handoff exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source ceiling"
            )));
        }
        if self
            .build_included_sources
            .iter()
            .any(|source| source.root() == included_root && source.relative_path() == relative_path)
        {
            return Err(Halt::Trap(
                "generated-source handoff names the same path more than once".to_owned(),
            ));
        }
        self.build_included_sources
            .push(crate::BuildIncludedSource::new(
                included_root,
                relative_path,
                self.filesystem_operation_attempts.len(),
            ));
        Ok(Value::Unit)
    }

    pub(in crate::interpreter::evaluator) fn exact_build_facet_method(
        &self,
        expected_attached: &str,
        expected_state: &str,
        target_symbol: SymbolHandle,
    ) -> bool {
        self.program.machines().iter().any(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == expected_attached)
                && self.symbol_has_build_prelude_source(machine.symbol)
                && self.program.machine_states(machine).iter().any(|state| {
                    state.name.as_str() == expected_state
                        && self.symbol_has_build_prelude_source(state.symbol)
                        // Build evaluation consumes the specialized typed tree
                        // before checked authored selections are finalized, so
                        // an attached value call may not yet carry its target
                        // symbol. The receiver above must still be the private
                        // compiler-created activation marker, and this search
                        // admits only the exact toolchain declaration.
                        && (!target_symbol.is_valid() || state.symbol == target_symbol)
                })
        })
    }

    pub(in crate::interpreter::evaluator) fn symbol_has_build_prelude_source(
        &self,
        symbol: SymbolHandle,
    ) -> bool {
        self.program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| self.program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file.path == std::path::Path::new("<build-prelude>")
            })
    }
}

/// Only private activation values carry rooted host authority. The public
/// empty BuildSource/BuildOutput stubs cannot manufacture this identity.
fn build_root_facet_identity(
    value: &Value,
    expected: &str,
) -> EvalResult<FilesystemGrantRootIdentity> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Err(Halt::Trap(
            "Build path operation requires a compiler-issued facet".into(),
        ));
    };
    if &**type_name != expected {
        return Err(Halt::Trap(
            "Build path operation received a different activation facet".into(),
        ));
    }
    fields
        .get("root")
        .and_then(|root| root.borrow().as_int())
        .and_then(|root| u32::try_from(root).ok())
        .and_then(FilesystemGrantRootIdentity::new)
        .ok_or_else(|| Halt::Trap("build-root activation facet has no valid root identity".into()))
}

pub(in crate::interpreter::evaluator) fn rooted_build_path_parts(
    value: &Value,
) -> EvalResult<Option<(FilesystemGrantRootIdentity, Vec<u8>)>> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Ok(None);
    };
    if **type_name != *ROOTED_BUILD_PATH_TYPE {
        return Ok(None);
    }
    let root = fields
        .get("root")
        .and_then(|root| root.borrow().as_int())
        .and_then(|root| u32::try_from(root).ok())
        .and_then(FilesystemGrantRootIdentity::new)
        .ok_or_else(|| Halt::Trap("rooted build path has no valid root identity".to_owned()))?;
    let relative = fields
        .get("relative")
        .and_then(|relative| match &*relative.borrow() {
            Value::Str(bytes) => Some(bytes.borrow().to_vec()),
            _ => None,
        })
        .ok_or_else(|| Halt::Trap("rooted build path has no relative bytes".to_owned()))?;
    Ok(Some((root, relative)))
}

pub(in crate::interpreter::evaluator) fn validate_build_relative_path(
    relative: &[u8],
) -> EvalResult<()> {
    if relative.is_empty() {
        return Err(Halt::Trap("build-root path is empty".to_owned()));
    }
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT {
        return Err(Halt::Resource(format!(
            "build-root path exceeds its {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}-byte ceiling"
        )));
    }
    if relative.contains(&0) {
        return Err(Halt::Trap("build-root path contains NUL".to_owned()));
    }
    if std::str::from_utf8(relative).is_err() {
        return Err(Halt::Trap(
            "build-root path must use canonical UTF-8 components".to_owned(),
        ));
    }
    if relative[0] == b'/'
        || relative.contains(&b'\\')
        || (relative.len() >= 2 && relative[1] == b':')
    {
        return Err(Halt::Trap(
            "build-root path must not use an absolute or host-specific spelling".to_owned(),
        ));
    }
    if relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
    {
        return Err(Halt::Trap(
            "build-root path must use canonical relative components".to_owned(),
        ));
    }
    Ok(())
}
