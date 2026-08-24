use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn new(program: &'program TypedTrees, stdin: &'program [u8]) -> Self {
        Self {
            program,
            operator_facts: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdin,
            stdin_cursor: 0,
            virtual_ticks: 0,
            virtual_live_windows: std::collections::HashSet::new(),
            virtual_window_next: 0,
            virtual_files: BTreeMap::new(),
            virtual_fds: BTreeMap::new(),
            virtual_next_fd: 3,
            virtual_dirs: std::collections::BTreeSet::new(),
            virtual_finds: BTreeMap::new(),
            virtual_next_find: 1,
            virtual_perms: BTreeMap::new(),
            virtual_symlinks: BTreeMap::new(),
            virtual_times: BTreeMap::new(),
            virtual_flocks: BTreeMap::new(),
            virtual_char_devices: [b"/dev/null".to_vec(), b"/dev/zero".to_vec()]
                .into_iter()
                .collect(),
            virtual_errno: 0,
            real_fs: None,
            host_boundary_touched: false,
            non_fs_host_boundary_touched: false,
            filesystem_operation_attempts: Vec::new(),
            usage: EvaluationUsage::empty(),
            // OMEGA_INTERP_STEP_BUDGET overrides the default for
            // measurement / long-running sample runs (dev knob, same
            // convention as the OMEGA_DEBUG_* flags); unset = the default.
            step_budget: std::env::var("OMEGA_INTERP_STEP_BUDGET")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(STEP_BUDGET),
            call_depth: 0,
            guard_depth: 0,
        }
    }

    pub(super) fn new_checked(checked: &'program CheckedTrees, stdin: &'program [u8]) -> Self {
        let mut evaluator = Self::new(&checked.typed, stdin);
        evaluator.operator_facts = Some(&checked.facts.operators);
        evaluator
    }

    pub(super) fn tick(&mut self) -> EvalResult<()> {
        self.usage
            .charge_step()
            .ok_or_else(|| Halt::Trap("evaluator usage overflowed".to_owned()))?;
        if self.usage.fuel_units() > self.step_budget {
            return trap("step budget exceeded");
        }
        Ok(())
    }

    // ---- entry --------------------------------------------------------------

    pub(super) fn run_entry(&mut self, entry_machine_name: &str) -> EvalResult<()> {
        let entry_machine = self
            .find_machine_by_name(entry_machine_name)
            .ok_or_else(|| Halt::Unsupported(format!("no entry machine `{entry_machine_name}`")))?
            .clone();
        let entry_state_name = self
            .machine_entry_state_name(&entry_machine)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "entry machine `{entry_machine_name}` has no executable state"
                ))
            })?;

        let instance = self.instantiate_machine(&entry_machine)?;
        // The entry machine's value (its terminal `Value` transition / final expression)
        // becomes the process exit code when it has no explicit `exit_process`. Mirrors the
        // backend: `machine Main::main(...) -> i32` returns the exit status.
        let returned =
            self.run_state_collect(&entry_machine, &entry_state_name, instance, Vec::new())?;
        if let Some(value) = returned {
            if let Some(code) = value.as_int() {
                return Err(Halt::Exit(code as i32));
            }
        }
        Ok(())
    }

    /// CONST EVALUATION: run `machine_name` (zero arguments, fresh default
    /// instance) to its terminal value. The machine's declared integer return
    /// type fixes the result's width semantics via `wrap_to_width` (target
    /// widths, never host widths). Non-integer terminal values are errors.
    pub(super) fn run_const_machine(&mut self, machine_name: &str) -> EvalResult<i64> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        // The declared INTEGER return type fixes the result's width semantics
        // (checked before running so the diagnostic names the type, not
        // whatever value the body happened to produce).
        let return_primitive = match self
            .find_state(&machine, &entry_state_name)
            .and_then(|state| self.program.primitive_type_reference(state.return_type))
        {
            Some(primitive)
                if primitive != PrimitiveType::Bool
                    && primitive != PrimitiveType::F32
                    && primitive != PrimitiveType::F64 =>
            {
                primitive
            }
            Some(primitive) => {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` returns `{}`, not an integer type",
                    primitive.name()
                )));
            }
            None => {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` does not declare an integer return type"
                )));
            }
        };

        let instance = self.instantiate_machine(&machine)?;
        let returned = self.run_state_collect(&machine, &entry_state_name, instance, Vec::new())?;
        let value = returned.ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` terminated without producing a value"
            ))
        })?;
        let raw = value.as_int().ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` produced a non-integer value"
            ))
        })?;

        Ok(wrap_to_width(raw, return_primitive))
    }

    /// STRUCTURED build-time evaluation: bind compiler-built arguments to the
    /// machine's entry-state parameters positionally, run to the terminal
    /// value, and deep-read it back out. Argument-count mismatch is a clear
    /// error here (the position's diagnostic names the machine); the caller
    /// owns the purity gate.
    pub(super) fn run_build_time_machine(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
    ) -> EvalResult<crate::build_time::BuildTimeValue> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        // The `&self` receiver is bound from the machine instance, not the
        // argument list -- exclude it from the positional count.
        let parameter_count = self
            .find_state(&machine, &entry_state_name)
            .map(|state| {
                self.program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| parameter.name.as_str() != "self")
                    .count()
            })
            .unwrap_or(0);
        if parameter_count != arguments.len() {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` takes {parameter_count} argument(s); the build-time \
                 position supplied {}",
                arguments.len()
            )));
        }

        let instance = self.instantiate_machine(&machine)?;
        let argument_cells = arguments
            .into_iter()
            .map(|argument| EvaluatedArgument::plain(argument.into_value().cell()))
            .collect();
        let returned =
            self.run_state_collect(&machine, &entry_state_name, instance, argument_cells)?;
        let value = returned.ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` terminated without producing a value"
            ))
        })?;
        // The dynamic purity backstop: the static effect surface does not yet
        // fold host-authority audit facts (boundary-trait calls), so any run
        // that actually touched the host is rejected here.
        if self.host_boundary_touched {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` is not effect-free: it drove a host-boundary call \
                 during build-time evaluation"
            )));
        }
        Ok(crate::build_time::BuildTimeValue::from_value(&value))
    }

    /// The augmenting-machine variant: run and read back the FINAL argument
    /// values (a `&mut` parameter aliases its argument cell, so mutations land
    /// there). A unit terminal is accepted -- the machine's OUTPUT is its
    /// arguments.
    pub(super) fn run_build_time_machine_arguments(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
    ) -> EvalResult<Vec<crate::build_time::BuildTimeValue>> {
        self.run_build_machine_arguments_with_policy(machine_name, arguments, false)
    }

    /// The shared augmenting-machine runner. `allow_filesystem` selects the
    /// dynamic backstop: `false` = the PURE build-time entry (any host touch
    /// rejects -- decision 12's discipline); `true` = the GRANTED build entry
    /// (filesystem ops are the point; every OTHER host boundary rejects).
    pub(super) fn run_build_machine_arguments_with_policy(
        &mut self,
        machine_name: &str,
        arguments: Vec<crate::build_time::BuildTimeValue>,
        allow_filesystem: bool,
    ) -> EvalResult<Vec<crate::build_time::BuildTimeValue>> {
        let machine = self
            .find_machine_by_name(machine_name)
            .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?
            .clone();
        let entry_state_name = self.machine_entry_state_name(&machine).ok_or_else(|| {
            Halt::Trap(format!(
                "machine `{machine_name}` has no states to evaluate"
            ))
        })?;
        let parameter_count = self
            .find_state(&machine, &entry_state_name)
            .map(|state| {
                self.program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| parameter.name.as_str() != "self")
                    .count()
            })
            .unwrap_or(0);
        if parameter_count != arguments.len() {
            return Err(Halt::Trap(format!(
                "machine `{machine_name}` takes {parameter_count} argument(s); the build-time position supplied {}",
                arguments.len()
            )));
        }

        let instance = self.instantiate_machine(&machine)?;
        let argument_cells: Vec<Cell> = arguments
            .into_iter()
            .map(|argument| argument.into_value().cell())
            .collect();
        // Keep the cells: a `&mut` parameter aliases its cell, so the run's
        // mutations are visible here afterward.
        let kept: Vec<Cell> = argument_cells.clone();
        let evaluated_arguments = argument_cells
            .into_iter()
            .map(EvaluatedArgument::plain)
            .collect();
        let _terminal =
            self.run_state_collect(&machine, &entry_state_name, instance, evaluated_arguments)?;
        let impure = if allow_filesystem {
            self.non_fs_host_boundary_touched
        } else {
            self.host_boundary_touched
        };
        if impure {
            return Err(Halt::Trap(if allow_filesystem {
                format!(
                    "machine `{machine_name}` drove a NON-filesystem host-boundary call during \
                     granted build evaluation -- only the Filesystem capability is granted"
                )
            } else {
                format!(
                    "machine `{machine_name}` is not effect-free: it drove a host-boundary call during build-time evaluation"
                )
            }));
        }
        Ok(kept
            .iter()
            .map(|cell| crate::build_time::BuildTimeValue::from_value(&cell.borrow()))
            .collect())
    }

    // ---- machine / data instantiation --------------------------------------

    /// Build a machine instance as a `Struct` whose fields are the attached data's
    /// fields (with their defaults) plus the machine's contained sub-objects.
    fn instantiate_machine(&mut self, machine: &Machine) -> EvalResult<Cell> {
        let mut fields: BTreeMap<String, Cell> = BTreeMap::new();

        if let Some(data_name) = machine.attached_data.as_ref() {
            if let Some(data) = self.find_data_by_name(data_name.as_str()) {
                self.populate_data_fields(data, &mut fields)?;
            }
        }

        // Machine-owned data (the `owned_data` span) are additional named cells.
        for owned in self.program.machine_owned_data(machine) {
            let value = if owned.initial_value.is_valid() {
                let frame = Frame {
                    locals: RefCell::new(BTreeMap::new()),
                    type_locals: RefCell::new(BTreeMap::new()),
                    self_cell: Value::Unit.cell(),
                    machine_symbol: SymbolHandle::invalid(),
                    scalar_locals: RefCell::new(BTreeMap::new()),
                    mutable_scalar_recasts: RefCell::new(BTreeMap::new()),
                    guard_call_results: RefCell::new(Vec::new()),
                };
                self.eval_expression(owned.initial_value, &frame)?
            } else {
                self.default_value_for_type(owned.type_reference)?
            };
            fields.insert(owned.name.as_str().to_owned(), value.cell());
        }

        Ok(Value::Struct {
            type_symbol: machine.symbol,
            type_name: machine.name.as_str().to_owned(),
            fields,
        }
        .cell())
    }

    /// Insert a `data` definition's fields (with defaults) into `fields`. Nested `data`
    /// members recurse so their own defaults are populated.
    pub(super) fn populate_data_fields(
        &mut self,
        data: &DataDefinition,
        fields: &mut BTreeMap<String, Cell>,
    ) -> EvalResult<()> {
        self.populate_data_fields_with_bindings(data, fields, &[])
    }

    fn populate_data_fields_with_bindings(
        &mut self,
        data: &DataDefinition,
        fields: &mut BTreeMap<String, Cell>,
        bindings: &[(
            SymbolHandle,
            String,
            psi_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> EvalResult<()> {
        let members = self.program.data_members(data).to_vec();
        for member in &members {
            let DataMember::Field(field) = member else {
                continue;
            };
            let name = field.name.as_str().to_owned();
            // Field defaults are retired: every field ZII zero-initializes.
            let value =
                self.default_value_for_type_with_bindings(field.type_reference, bindings)?;
            fields.insert(name, value.cell());
        }
        Ok(())
    }

    /// Build a default-initialized value for a declared type, recursing into nested `data`
    /// records (a sub-Struct with its own defaults) and fixed arrays (an `Array` of
    /// per-element default cells). Falls back to the primitive/unit default.
    pub(super) fn default_value_for_type(
        &mut self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> EvalResult<Value> {
        self.default_value_for_type_with_bindings(type_reference, &[])
    }

    fn default_value_for_type_with_bindings(
        &mut self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
        bindings: &[(
            SymbolHandle,
            String,
            psi_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> EvalResult<Value> {
        if type_reference.is_valid() {
            if let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
                && let Some(argument) =
                    self.generic_binding_argument(*symbol, name.as_str(), bindings)
            {
                return self.default_value_for_type_with_bindings(argument, bindings);
            }

            // An owned `[u8; N] in <text-domain>` is not an always-full fixed
            // array. Its runtime representation is the bounded carrier
            // `{ len, bytes[N] }`, whose honest ZII value has `len == 0`.
            // Recognize that shape BEFORE stripping the domain constraint;
            // otherwise the interpreter constructs N zero-valued array
            // elements and compares it against text literals as a non-text
            // value, diverging from both the checked semantics and native
            // `BoundedByteBuffer` layout.
            if self.declared_type_is_bounded_byte_buffer(type_reference) {
                return Ok(Value::bytes(Vec::new()));
            }

            // See THROUGH a domain constraint (`[i32; N] in Wrapping`, `i32 in Saturating`):
            // the default of a constrained type is the default of its base type (zero in every
            // arithmetic domain). Without this, a domain-constrained ARRAY field falls past the
            // FixedArray case below and defaults to `Unit`, so a later `self.arr[i]` raised
            // "cannot index Unit" and the whole canary was SKIPPED by the differential oracle.
            if let psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let base_type = *base_type;
                return self.default_value_for_type_with_bindings(base_type, bindings);
            }

            // Fixed array `[T; N]` -> N default-initialized element cells.
            if let psi_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let element_type = *element_type;
                let count = match length {
                    psi_typed_trees::types::FixedArrayLength::Literal(count) => Some(*count),
                    psi_typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => {
                        self.generic_binding_argument(*symbol, name.as_str(), bindings)
                            .and_then(|argument| {
                                self.const_argument_value_with_bindings(argument, bindings, 0)
                            })
                    }
                    psi_typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
                };
                if let Some(count) = count {
                    let mut elements = Vec::with_capacity(count);
                    for _ in 0..count {
                        elements.push(
                            self.default_value_for_type_with_bindings(element_type, bindings)?
                                .cell(),
                        );
                    }
                    return Ok(Value::Array(elements));
                }
            }

            if let psi_typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } = self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                let definition = self
                    .program
                    .data_definitions()
                    .iter()
                    .find(|data| {
                        (base_symbol.is_valid() && data.symbol == *base_symbol)
                            || data.name.as_str() == base_name.as_str()
                    })
                    .cloned();
                if let Some(definition) = definition
                    && matches!(
                        DataDefinition::shape_kind_from_members(
                            self.program.data_members(&definition)
                        ),
                        psi_typed_trees::data::DataShapeKind::Record
                    )
                {
                    let parameters = self.program.data_type_parameters(&definition).to_vec();
                    let arguments = self
                        .program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec();
                    let mut nested_bindings = bindings.to_vec();
                    nested_bindings.extend(parameters.iter().zip(arguments).map(
                        |(parameter, argument)| {
                            (
                                parameter.symbol,
                                parameter.name.as_str().to_owned(),
                                argument,
                            )
                        },
                    ));
                    let mut nested_fields = BTreeMap::new();
                    self.populate_data_fields_with_bindings(
                        &definition,
                        &mut nested_fields,
                        &nested_bindings,
                    )?;
                    return Ok(Value::Struct {
                        type_symbol: definition.symbol,
                        type_name: definition.name.as_str().to_owned(),
                        fields: nested_fields,
                    });
                }
            }

            // Nested `data` record -> a sub-Struct of its own defaults.
            if let Some(nested) = self.field_nested_data(type_reference) {
                let mut nested_fields = BTreeMap::new();
                self.populate_data_fields(nested, &mut nested_fields)?;
                return Ok(Value::Struct {
                    type_symbol: nested.symbol,
                    type_name: nested.name.as_str().to_owned(),
                    fields: nested_fields,
                });
            }

            // An enum-shaped field defaults to the ZERO CASE (ZII: tag 0 is
            // the first case) with the case's payload fields zeroed --
            // matching native zero-initialized storage, so tag compares and
            // synthesized structural equality agree on never-assigned sum
            // fields instead of seeing a Unit placeholder.
            if let Some((type_symbol, variant_name, payload_fields)) =
                self.enum_zero_case(type_reference)
            {
                let mut payload = Vec::with_capacity(payload_fields.len());
                for field in payload_fields {
                    let value =
                        self.default_value_for_type_with_bindings(field.type_reference, bindings)?;
                    payload.push((field.name.as_str().to_owned(), value.cell()));
                }
                return Ok(Value::Enum {
                    type_symbol,
                    variant_name,
                    payload,
                });
            }
        }
        Ok(self.default_for_type(type_reference))
    }

    fn generic_binding_argument(
        &self,
        symbol: SymbolHandle,
        name: &str,
        bindings: &[(
            SymbolHandle,
            String,
            psi_typed_trees::types::TypeReferenceHandle,
        )],
    ) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
        bindings
            .iter()
            .find(|(parameter, spelling, _)| {
                if symbol.is_valid() {
                    *parameter == symbol
                } else {
                    spelling == name
                }
            })
            .map(|(_, _, argument)| *argument)
    }

    fn const_argument_value_with_bindings(
        &self,
        argument: psi_typed_trees::types::TypeReferenceHandle,
        bindings: &[(
            SymbolHandle,
            String,
            psi_typed_trees::types::TypeReferenceHandle,
        )],
        depth: usize,
    ) -> Option<usize> {
        if depth >= 16 {
            return None;
        }
        let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } =
            self.program.type_reference_table.type_reference(argument)
        else {
            return None;
        };
        if !symbol.is_valid() {
            if let Ok(value) = name.as_str().parse::<usize>() {
                return Some(value);
            }
        }
        let argument = self.generic_binding_argument(*symbol, name.as_str(), bindings)?;
        self.const_argument_value_with_bindings(argument, bindings, depth + 1)
    }

    /// The first case of a case-bearing declared type (the ZII zero case),
    /// with the field declarations a zeroed value carries: the COMMON fields
    /// (mixed shapes -- present in every case) followed by the zero case's
    /// payload fields.
    fn enum_zero_case(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Option<(SymbolHandle, String, Vec<psi_typed_trees::data::DataField>)> {
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return None;
        }
        let symbol = self.program.type_reference_symbol(type_reference);
        if !symbol.is_valid() {
            return None;
        }
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)?;
        let members = self.program.data_members(data);
        let first_variant = members.iter().find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            _ => None,
        })?;
        let mut fields: Vec<psi_typed_trees::data::DataField> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field.clone()),
                _ => None,
            })
            .collect();
        fields.extend(self.program.data_payload_fields(first_variant).to_vec());
        Some((data.symbol, first_variant.name.as_str().to_owned(), fields))
    }

    /// If a field's declared type is a (non-primitive) `data` record, return it.
    fn field_nested_data(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Option<&'program DataDefinition> {
        if !type_reference.is_valid() {
            return None;
        }
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return None;
        }
        let symbol = self.program.type_reference_symbol(type_reference);
        if !symbol.is_valid() {
            return None;
        }
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
            .filter(|data| {
                // Records instantiate as nested structs; case-bearing data (sums
                // AND mixed shapes) doesn't -- it zero-initializes to the first
                // case via `enum_zero_case`. EMPTY records (e.g. `data Circle {}`)
                // must still instantiate as a typed struct -- the type identity
                // is what a `dyn Trait` receiver dispatches on at runtime.
                !matches!(
                    DataDefinition::shape_kind_from_members(self.program.data_members(data)),
                    psi_typed_trees::data::DataShapeKind::Enum
                        | psi_typed_trees::data::DataShapeKind::Mixed
                )
            })
    }

    pub(super) fn default_for_type(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Value {
        match self.program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::Bool) => Value::Bool(false),
            Some(PrimitiveType::F32) | Some(PrimitiveType::F64) => Value::Float(0.0),
            Some(_) => Value::Int(0),
            // A `&[u8] in Utf8` text view (the encoding-domain model that retires
            // the retired owned primitive, #66) uses a fat `{ptr,len}` descriptor
            // and the content-compare/literal-store path natively. The
            // zero-initialized field is a zeroed descriptor (empty bytes), so the
            // interpreter must default it to an EMPTY `Str` -- not `Unit`. (A `Unit`
            // default makes `self.name == "literal"` fall through `values_equal`'s
            // int-compare arm where `None == None` is spuriously TRUE, diverging from
            // the native empty-vs-nonempty content compare.)
            None if self.program.is_borrowed_byte_slice(type_reference) => {
                Value::str(String::new())
            }
            None => Value::Unit,
        }
    }

    // ---- state execution ----------------------------------------------------

    /// Run a state; returns the value produced by a `Value` transition target, if any.
    /// Guards native recursion depth so a deeply recursive program is declined (skipped)
    /// instead of overflowing the host stack.
    pub(super) fn run_state_collect(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    ) -> EvalResult<Option<Value>> {
        self.call_depth += 1;
        if self.call_depth > CALL_DEPTH_BUDGET {
            self.call_depth -= 1;
            return unsupported("recursion depth budget exceeded");
        }
        let result = self.run_state_collect_inner(machine, state_name, instance, args);
        self.call_depth -= 1;
        result
    }

    fn run_state_collect_inner(
        &mut self,
        machine: &Machine,
        state_name: &str,
        instance: Cell,
        args: Vec<EvaluatedArgument>,
    ) -> EvalResult<Option<Value>> {
        // MR4 admission: the cross-machine tail transition REBINDS these and
        // continues the loop (a jump, mirroring the native dispatch-loop
        // lowering) instead of recursing -- an admitted measured mutual
        // cycle must not consume interpreter call depth.
        let mut machine = machine.clone();
        let mut instance = instance;
        let mut current_state = state_name.to_owned();
        let mut current_args = args;
        // Locals accumulated across SAME-machine sibling transitions: the backend models a
        // machine as one frame whose slots persist, so an inlined sub-state still sees the
        // enclosing state's params/`let`s (e.g. `mark_current_room` reading `enter_room`'s
        // `room_index`). New args bind on top; carried-over names stay visible.
        let mut carried: BTreeMap<String, Cell> = BTreeMap::new();
        let mut carried_types: BTreeMap<String, TypeReferenceHandle> = BTreeMap::new();
        let mut carried_recasts: BTreeMap<String, MutableScalarRecast> = BTreeMap::new();

        loop {
            self.tick()?;
            let state = self
                .find_state(&machine, &current_state)
                .ok_or_else(|| Halt::Unsupported(format!("unknown state `{current_state}`")))?
                .clone();

            let frame = self.bind_frame(
                &state,
                Rc::clone(&instance),
                &current_args,
                machine.symbol,
                &carried,
                &carried_types,
                &carried_recasts,
            )?;

            // Execute statements, watching for the first satisfied transition. A state
            // whose body ends in a bare expression (`{ 22 }`) returns that expression's
            // value as its result (the backend's value-state form).
            let mut next: Option<TransitionDecision> = None;
            let mut tail_value: Option<Value> = None;
            for statement in self
                .program
                .statement_table
                .statements(state.statement_nodes)
            {
                let statement = statement.clone();
                match &statement {
                    StatementNode::Transition(transition) => {
                        if let Some(decision) = self.eval_transition(transition, &frame)? {
                            next = Some(decision);
                            break;
                        }
                    }
                    StatementNode::Expression(expression) => {
                        tail_value = Some(self.eval_expression(*expression, &frame)?);
                    }
                    other => {
                        self.exec_statement(other, &frame)?;
                    }
                }
            }

            match next {
                None => return Ok(tail_value),
                Some(TransitionDecision::Value(value)) => return Ok(Some(value)),
                Some(TransitionDecision::Terminal) => return Ok(None),
                Some(TransitionDecision::SelfTarget) => {
                    // Re-run the same state (rare; guard against infinite loops via budget),
                    // carrying its bindings forward.
                    carried = frame.locals.into_inner();
                    carried_types = frame.type_locals.into_inner();
                    carried_recasts = frame.mutable_scalar_recasts.into_inner();
                    continue;
                }
                Some(TransitionDecision::Named {
                    state_name,
                    machine: target_machine,
                    instance: target_instance,
                    args,
                }) => {
                    if target_machine.symbol == machine.symbol
                        && Rc::ptr_eq(&target_instance, &instance)
                    {
                        // Carry this state's bindings forward to the sibling state.
                        carried = frame.locals.into_inner();
                        carried_types = frame.type_locals.into_inner();
                        carried_recasts = frame.mutable_scalar_recasts.into_inner();
                        current_state = state_name;
                        current_args = args;
                        continue;
                    }
                    // Cross-machine named transition: a TAIL JUMP into the
                    // target machine (the arm target is the arm's last
                    // action; whichever machine terminates delivers the
                    // value). Rebind the loop -- constant depth, matching
                    // the native SetDispatchState lowering. The carried
                    // locals clear: the callee binds a fresh frame.
                    machine = target_machine;
                    instance = target_instance;
                    current_state = state_name;
                    current_args = args;
                    carried = BTreeMap::new();
                    carried_types = BTreeMap::new();
                    carried_recasts = BTreeMap::new();
                    continue;
                }
            }
        }
    }

    /// Bind a state's parameters (skipping `self`) to the positional argument cells. Seeds
    /// from `carried` (the enclosing same-machine state's bindings) so an inlined sub-state
    /// still sees outer params/locals; the state's own params override on top.
    fn bind_frame(
        &self,
        state: &State,
        self_cell: Cell,
        args: &[EvaluatedArgument],
        machine_symbol: SymbolHandle,
        carried: &BTreeMap<String, Cell>,
        carried_types: &BTreeMap<String, TypeReferenceHandle>,
        carried_recasts: &BTreeMap<String, MutableScalarRecast>,
    ) -> EvalResult<Frame> {
        let mut scalar_locals = BTreeMap::new();
        let mut locals = carried.clone();
        let mut type_locals = carried_types.clone();
        let mut mutable_scalar_recasts = carried_recasts.clone();
        let mut arg_index = 0;
        for parameter in self.program.state_parameters(state) {
            if parameter.is_self {
                continue;
            }
            let argument = args
                .get(arg_index)
                .cloned()
                .unwrap_or_else(|| EvaluatedArgument::plain(Value::Unit.cell()));
            let cell = argument.cell;
            // Coerce a by-value ARGUMENT to the param's declared width/domain at
            // the binding, matching the native truncating/clamping/trapping store
            // at the call boundary. Mirrors the Assignment/LocalData store wraps:
            //   * f32 param: round a `Float` to f32 (an inline `+1.0` arg is f64;
            //     native passes it in an f32 register).
            //   * integer param: wrap/clamp/trap an `Int` to the param's width +
            //     arithmetic domain (a u8 param given `a+b`=300 must read 44).
            // A `&mut` arg carries a `Ref`/place (not a `Float`/`Int`), so it is
            // left untouched and its aliasing preserved (keep the original cell);
            // a by-value scalar is a copy anyway, so a fresh coerced cell is
            // correct. Funnels through `coerce_scalar_with` like every other seam.
            // The resolved (primitive, domain) is also RECORDED so arithmetic on
            // the param applies its declared domain at the operation node.
            let cell = match self
                .program
                .primitive_type_reference(parameter.type_reference)
            {
                Some(primitive) => {
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(parameter.type_reference);
                    scalar_locals.insert(parameter.name.as_str().to_owned(), (primitive, domain));
                    let scalar = match &*cell.borrow() {
                        v @ (Value::Int(_) | Value::Float(_)) => Some(v.clone()),
                        _ => None,
                    };
                    match scalar {
                        Some(value) => self.coerce_scalar_with(value, primitive, domain)?.cell(),
                        None => cell,
                    }
                }
                None => cell,
            };
            locals.insert(parameter.name.as_str().to_owned(), cell);
            type_locals.insert(parameter.name.as_str().to_owned(), parameter.type_reference);
            if let Some(recast) = argument.mutable_recast {
                mutable_scalar_recasts.insert(parameter.name.as_str().to_owned(), recast);
            }
            arg_index += 1;
        }
        Ok(Frame {
            locals: RefCell::new(locals),
            type_locals: RefCell::new(type_locals),
            self_cell,
            machine_symbol,
            scalar_locals: RefCell::new(scalar_locals),
            mutable_scalar_recasts: RefCell::new(mutable_scalar_recasts),
            guard_call_results: RefCell::new(Vec::new()),
        })
    }
}
