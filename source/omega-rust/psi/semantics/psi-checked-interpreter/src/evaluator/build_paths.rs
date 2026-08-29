use super::*;
use crate::{
    FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, FilesystemGrantRootIdentity,
    MAX_INCLUDED_BUILD_SOURCES,
};

pub(super) const ROOTED_BUILD_PATH_TYPE: &str = "$OmegaBuildRootedPath";
const SOURCE_ROOT_FACET_TYPE: &str = "$OmegaBuildSourceRoot";
const OUTPUT_ROOT_FACET_TYPE: &str = "$OmegaBuildOutputRoot";

impl<'program> Evaluator<'program> {
    pub(super) fn enable_rooted_build_paths_from_arguments(
        &mut self,
        arguments: &[crate::build_time::BuildTimeValue],
    ) {
        self.rooted_build_paths_required = arguments.iter().any(|argument| {
            let crate::build_time::BuildTimeValue::Struct { fields, .. } = argument else {
                return false;
            };
            fields.iter().any(|(_, value)| {
                matches!(
                    value,
                    crate::build_time::BuildTimeValue::Struct { type_name, .. }
                        if matches!(type_name.as_str(), SOURCE_ROOT_FACET_TYPE | OUTPUT_ROOT_FACET_TYPE)
                )
            })
        });
    }

    pub(super) fn try_build_root_resolve_value_call(
        &mut self,
        call: &psi_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "resolve" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        let (expected_attached, root) = match &*receiver.borrow() {
            Value::Struct {
                type_name, fields, ..
            } if matches!(
                type_name.as_str(),
                SOURCE_ROOT_FACET_TYPE | OUTPUT_ROOT_FACET_TYPE
            ) =>
            {
                let attached = if type_name == SOURCE_ROOT_FACET_TYPE {
                    "BuildSource"
                } else {
                    "BuildOutput"
                };
                let root = fields
                    .get("root")
                    .and_then(|root| root.borrow().as_int())
                    .and_then(|root| u32::try_from(root).ok())
                    .and_then(FilesystemGrantRootIdentity::new)
                    .ok_or_else(|| {
                        Halt::Trap(
                            "build-root activation facet has no valid root identity".to_owned(),
                        )
                    })?;
                (attached, root)
            }
            _ => return Ok(None),
        };
        if !self.exact_build_root_resolver(expected_attached, call.target_symbol) {
            return Err(Halt::Trap(
                "build-root resolution did not select the exact toolchain resolver".to_owned(),
            ));
        }
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        let [relative] = arguments else {
            return Err(Halt::Trap(
                "build-root resolution requires one relative path".to_owned(),
            ));
        };
        let relative = match self.eval_expression(*relative, frame)? {
            Value::Str(bytes) => bytes.borrow().clone(),
            Value::Array(cells) => cells
                .iter()
                .map(|cell| {
                    let value = cell.borrow();
                    value
                        .as_int()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| {
                            Halt::Trap("build-root path contains a non-byte element".to_owned())
                        })
                })
                .collect::<EvalResult<Vec<_>>>()?,
            other => {
                return Err(Halt::Trap(format!(
                    "build-root path must be byte data, got {other:?}"
                )));
            }
        };
        validate_build_relative_path(&relative)?;
        Ok(Some(Value::Struct {
            type_symbol: SymbolHandle::invalid(),
            type_name: ROOTED_BUILD_PATH_TYPE.to_owned(),
            fields: BTreeMap::from([
                ("root".to_owned(), Value::Int(i64::from(root.get())).cell()),
                ("relative".to_owned(), Value::bytes(relative).cell()),
            ]),
        }))
    }

    pub(super) fn try_build_output_include_source_value_call(
        &mut self,
        call: &psi_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if call.target.as_str() != "include_source" || !call.receiver.is_valid() {
            return Ok(None);
        }
        let receiver = self.resolve_place(call.receiver, frame)?;
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        self.try_include_source(receiver, call.target_symbol, arguments, frame)
            .map(|handled| handled.then_some(Value::Unit))
    }

    pub(super) fn try_build_output_include_source_statement(
        &mut self,
        call: &psi_typed_trees::statement::TableCall,
        frame: &Frame,
    ) -> EvalResult<bool> {
        if call.target.as_str() != "include_source" || call.receiver.is_empty() {
            return Ok(false);
        }
        let Some(receiver) = self.statement_receiver_cell(call.receiver, frame)? else {
            return Ok(false);
        };
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        self.try_include_source(receiver, call.target_symbol, arguments, frame)
    }

    fn try_include_source(
        &mut self,
        receiver: Cell,
        target_symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<bool> {
        let receiver = self.deref_cell(receiver);
        let output_root = match &*receiver.borrow() {
            Value::Struct {
                type_name, fields, ..
            } if type_name == OUTPUT_ROOT_FACET_TYPE => fields
                .get("root")
                .and_then(|root| root.borrow().as_int())
                .and_then(|root| u32::try_from(root).ok())
                .and_then(FilesystemGrantRootIdentity::new)
                .ok_or_else(|| {
                    Halt::Trap(
                        "build-output activation facet has no valid root identity".to_owned(),
                    )
                })?,
            _ => return Ok(false),
        };
        if !self.exact_build_output_include_source(target_symbol) {
            return Err(Halt::Trap(
                "generated-source handoff did not select the exact toolchain machine".to_owned(),
            ));
        }
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
            .is_some_and(real_fs::RealFs::is_scoped);
        let replayed_output = self
            .filesystem_replay
            .as_ref()
            .and_then(|replay| {
                replay
                    .expected_included_sources()
                    .get(self.build_included_sources.len())
            })
            .is_some_and(|expected| {
                expected.root() == included_root
                    && expected.relative_path() == relative_path
                    && expected.filesystem_attempt_ordinal()
                        == self.filesystem_operation_attempts.len()
            });
        if !scoped_real_output && !replayed_output {
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
        Ok(true)
    }

    fn statement_receiver_cell(
        &self,
        receiver: psi_arena::HandleSpan<psi_typed_trees::name::Identifier>,
        frame: &Frame,
    ) -> EvalResult<Option<Cell>> {
        let members = self.program.statement_table.name_path_members(receiver);
        let Some(first) = members.first() else {
            return Ok(None);
        };
        let mut cell = Rc::clone(&frame.self_cell);
        let mut start = 0usize;
        if first.as_str() == "self" {
            start = 1;
        } else if let Some(local) = frame.get(first.as_str()) {
            cell = local;
            start = 1;
        }
        for member in &members[start..] {
            cell = self.deref_cell(cell);
            cell = match self.field_cell(&cell, member.as_str()) {
                Ok(field) => field,
                Err(_) => return Ok(None),
            };
        }
        Ok(Some(self.deref_cell(cell)))
    }

    fn exact_build_root_resolver(
        &self,
        expected_attached: &str,
        target_symbol: SymbolHandle,
    ) -> bool {
        self.program.machines().iter().any(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == expected_attached)
                && self.symbol_has_build_prelude_source(machine.symbol)
                && self.program.machine_states(machine).iter().any(|state| {
                    state.name.as_str() == "resolve"
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

    fn exact_build_output_include_source(&self, target_symbol: SymbolHandle) -> bool {
        self.program.machines().iter().any(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == "BuildOutput")
                && self.symbol_has_build_prelude_source(machine.symbol)
                && self.program.machine_states(machine).iter().any(|state| {
                    state.name.as_str() == "include_source"
                        && self.symbol_has_build_prelude_source(state.symbol)
                        && (!target_symbol.is_valid() || state.symbol == target_symbol)
                })
        })
    }

    fn symbol_has_build_prelude_source(&self, symbol: SymbolHandle) -> bool {
        self.program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| self.program.symbols.source_file(span))
            .is_some_and(|file| {
                file.origin == psi_source::SourceOrigin::Toolchain
                    && file.path == std::path::Path::new("<build-prelude>")
            })
    }
}

pub(super) fn rooted_build_path_parts(
    value: &Value,
) -> EvalResult<Option<(FilesystemGrantRootIdentity, Vec<u8>)>> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Ok(None);
    };
    if type_name != ROOTED_BUILD_PATH_TYPE {
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
            Value::Str(bytes) => Some(bytes.borrow().clone()),
            _ => None,
        })
        .ok_or_else(|| Halt::Trap("rooted build path has no relative bytes".to_owned()))?;
    Ok(Some((root, relative)))
}

pub(super) fn validate_build_relative_path(relative: &[u8]) -> EvalResult<()> {
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
