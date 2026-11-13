//! Read published contracts under exact source applications for the ordinary
//! callable-refinement witness. This never schedules a supplying body.

use super::{
    Program,
    machine::{Declarer, ValueBinder},
};
use crate::{
    CallableDeclaration, CallableRefinementContext,
    body::{CrashCeiling, Termination, ValueType},
};
use semantic_vocabulary::{MachineId, ScalarType};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::{
    data::{TypeParameter, TypeParameterKind},
    signature::{SignatureContract, StateParameter},
    types::TypeReferenceHandle,
};

struct Header<'t> {
    declaration: SymbolHandle,
    owner: SymbolHandle,
    lexical_owner: SymbolHandle,
    name: String,
    generics: Vec<TypeParameter>,
    parameters: &'t [StateParameter],
    result: TypeReferenceHandle,
    contracts: Vec<SignatureContract>,
    declared: Declarer,
}

impl<'t> Program<'t> {
    pub(super) fn refinement_contracts(
        &mut self,
        required: CallableDeclaration<'t>,
        supplied: CallableDeclaration<'t>,
        context: CallableRefinementContext<'_>,
    ) -> Checked<(MachineId, MachineId)> {
        let id = self.next_identity()?;
        let required = self.refinement_header(id, required, context.required_lexical_owner)?;
        let supplied = self.refinement_header(id, supplied, context.supplied_lexical_owner)?;
        if let Some(selecting) = context.selecting {
            let selection = self.typed.conformance_by_symbol(selecting).ok_or_else(|| {
                super::services::unavailable(
                    id,
                    "callable refinement lost its exact selecting implementation",
                )
            })?;
            if !selection
                .rows
                .iter()
                .any(|row| row.realization_machine == supplied.declaration)
            {
                return Err(super::services::unavailable(
                    id,
                    "callable refinement selection names another supplying declaration",
                ));
            }
        }
        let previous = std::mem::take(&mut self.scope);
        let result = (|| {
            let mut type_pairs = Vec::new();
            let mut value_pairs = Vec::new();
            for &(left, right) in context.binder_pairs {
                if type_pairs
                    .iter()
                    .chain(&value_pairs)
                    .any(|(prior_left, prior_right)| *prior_left == left || *prior_right == right)
                {
                    return Err(super::services::unavailable(
                        id,
                        "callable refinement repeats a declaration binder",
                    ));
                }
                let left_parameter = self.refinement_parameter(id, &required, left)?;
                let right_parameter = self.refinement_parameter(id, &supplied, right)?;
                match (&left_parameter.kind, &right_parameter.kind) {
                    (TypeParameterKind::Type, TypeParameterKind::Type) => {
                        let ordinal = u32::try_from(type_pairs.len()).map_err(|_| {
                            super::services::unavailable(
                                id,
                                "callable refinement type namespace exceeds its ordinal width",
                            )
                        })?;
                        let value =
                            ValueType::Rigid(terminal_check_tables::values::RigidType(ordinal));
                        self.scope.push((left, value));
                        if right != left {
                            self.scope.push((right, value));
                        }
                        type_pairs.push((left, right));
                    }
                    (TypeParameterKind::Const { .. }, TypeParameterKind::Const { .. })
                    | (TypeParameterKind::Value { .. }, TypeParameterKind::Value { .. }) => {
                        value_pairs.push((left, right))
                    }
                    _ => {
                        return Err(super::services::unavailable(
                            id,
                            "callable refinement binder kind has no represented correspondence",
                        ));
                    }
                }
            }
            // Supplier operands retain their source namespace; required operands
            // are then read in that same applied namespace.
            self.apply_source_type_frames(
                id,
                context.supplied_types,
                context.supplied_scope_starts,
                &type_pairs,
            )?;
            self.apply_source_type_frames(
                id,
                context.required_types,
                context.required_scope_starts,
                &type_pairs,
            )?;
            let mut required_bound = Vec::new();
            let mut supplied_bound = Vec::new();
            for (header, constants, required_side) in [
                (&required, context.required_constants, true),
                (&supplied, context.supplied_constants, false),
            ] {
                for (parameter, carrier, identity) in constants {
                    let value =
                        self.refinement_constant(id, header, *parameter, *carrier, identity)?;
                    let target = if required_side {
                        &mut required_bound
                    } else {
                        &mut supplied_bound
                    };
                    if target.iter().any(|(bound, _)| bound == parameter) {
                        return Err(super::services::unavailable(
                            id,
                            "callable refinement repeats a retained constant operand",
                        ));
                    }
                    target.push((*parameter, value));
                }
            }
            for &(left, right) in &value_pairs {
                let left_value = required_bound
                    .iter()
                    .find(|(bound, _)| *bound == left)
                    .map(|(_, value)| value.clone());
                let right_value = supplied_bound
                    .iter()
                    .find(|(bound, _)| *bound == right)
                    .map(|(_, value)| value.clone());
                match (left_value, right_value) {
                    (Some(left), Some(right)) if left != right => {
                        return Err(super::services::unavailable(
                            id,
                            "paired callable constant operands disagree",
                        ));
                    }
                    (Some(value), None) => supplied_bound.push((right, value)),
                    (None, Some(value)) => required_bound.push((left, value)),
                    _ => {}
                }
            }
            let mut required_values = Vec::new();
            let mut supplied_values = Vec::new();
            for (left, right) in value_pairs {
                let left_parameter = self.refinement_parameter(id, &required, left)?;
                let right_parameter = self.refinement_parameter(id, &supplied, right)?;
                let carrier = |parameter: &TypeParameter| match parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => Some(type_reference),
                    _ => None,
                };
                let left_type = self.refinement_scalar_type(
                    id,
                    carrier(&left_parameter).ok_or_else(|| {
                        super::services::unavailable(id, "value binder lost its carrier")
                    })?,
                )?;
                let right_type = self.refinement_scalar_type(
                    id,
                    carrier(&right_parameter).ok_or_else(|| {
                        super::services::unavailable(id, "value binder lost its carrier")
                    })?,
                )?;
                if left_type != right_type || !matches!(left_type, ScalarType::Integer(_)) {
                    return Err(super::services::unavailable(
                        id,
                        "callable refinement value binders have different or unrepresented carriers",
                    ));
                }
                if required_bound.iter().any(|(symbol, _)| *symbol == left) {
                    continue;
                }
                required_values.push(ValueBinder {
                    symbol: left,
                    name: left_parameter.name.to_string(),
                    scalar_type: left_type,
                });
                supplied_values.push(ValueBinder {
                    symbol: right,
                    name: right_parameter.name.to_string(),
                    scalar_type: right_type,
                });
            }
            for (header, values, bound) in [
                (&required, &required_values, &required_bound),
                (&supplied, &supplied_values, &supplied_bound),
            ] {
                for parameter in &header.generics {
                    let represented = match parameter.kind {
                        TypeParameterKind::Type => self
                            .scope
                            .iter()
                            .any(|(symbol, _)| *symbol == parameter.symbol),
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                            values.iter().any(|value| value.symbol == parameter.symbol)
                                || bound.iter().any(|(symbol, _)| *symbol == parameter.symbol)
                        }
                        _ => false,
                    };
                    if !represented {
                        return Err(super::services::unavailable(
                            id,
                            "callable refinement retains an unapplied static formal",
                        ));
                    }
                }
            }
            let scope = self.scope.clone();
            let entry = self.refinement_row(required, &required_values, &required_bound)?;
            self.scope = scope;
            let callee = self.refinement_row(supplied, &supplied_values, &supplied_bound)?;
            let required_header = terminal_check_tables::row(
                &self.signatures,
                entry,
                entry,
                terminal_check_tables::Construct::Machine,
            )?;
            let supplied_header = terminal_check_tables::row(
                &self.signatures,
                callee,
                entry,
                terminal_check_tables::Construct::Machine,
            )?;
            if matches!(supplied_header.crashes, CrashCeiling::Inferred)
                && !matches!(required_header.crashes, CrashCeiling::Inferred)
            {
                return Err(super::services::unavailable(
                    id,
                    "supplied public contract has an open crash ceiling",
                ));
            }
            self.requirements.push(callee);
            self.fixed_inputs.extend([entry, callee]);
            Ok((entry, callee))
        })();
        self.scope = previous;
        result
    }

    pub(super) fn apply_source_type_frames(
        &mut self,
        id: MachineId,
        bindings: &[(SymbolHandle, TypeReferenceHandle)],
        starts: &[usize],
        aliases: &[(SymbolHandle, SymbolHandle)],
    ) -> Checked<()> {
        if !starts.is_empty() && starts.len() != bindings.len() {
            return Err(super::services::unavailable(
                id,
                "callable refinement lost its substitution-frame boundaries",
            ));
        }
        let mut cursor = 0;
        while cursor < bindings.len() {
            let start = if starts.is_empty() { 0 } else { starts[cursor] };
            if start != cursor {
                return Err(super::services::unavailable(
                    id,
                    "callable refinement has a malformed substitution frame",
                ));
            }
            let end = if starts.is_empty() {
                bindings.len()
            } else {
                starts[cursor + 1..]
                    .iter()
                    .position(|next| *next != start)
                    .map_or(bindings.len(), |offset| cursor + offset + 1)
            };
            let frame_scope = self.scope.clone();
            let mut applied = Vec::new();
            for &(parameter, reference) in &bindings[cursor..end] {
                if !parameter.is_valid()
                    || applied.iter().any(|(known, _)| *known == parameter)
                    || !self
                        .typed
                        .type_reference_table
                        .contains_type_reference(reference)
                {
                    return Err(super::services::unavailable(
                        id,
                        "callable refinement has a stale or repeated type application",
                    ));
                }
                let declaration = self
                    .typed
                    .data_type_parameter_by_symbol(parameter)
                    .ok_or_else(|| {
                        super::services::unavailable(
                            id,
                            "callable refinement type application has no exact formal declaration",
                        )
                    })?;
                if declaration.kind != TypeParameterKind::Type {
                    return Err(super::services::unavailable(
                        id,
                        "callable refinement static operand requires its retained value application",
                    ));
                }
                self.scope = frame_scope.clone();
                applied.push((parameter, self.value_type(id, reference)?));
            }
            self.scope = frame_scope;
            for (parameter, value) in applied {
                let pair = aliases
                    .iter()
                    .find(|(left, right)| *left == parameter || *right == parameter)
                    .copied();
                for symbol in std::iter::once(parameter)
                    .chain(pair.into_iter().flat_map(|(left, right)| [left, right]))
                {
                    if let Some((_, existing)) =
                        self.scope.iter_mut().find(|(bound, _)| *bound == symbol)
                    {
                        *existing = value;
                    } else {
                        self.scope.push((symbol, value));
                    }
                }
            }
            cursor = end;
        }
        Ok(())
    }

    fn refinement_parameter(
        &self,
        id: MachineId,
        header: &Header<'t>,
        symbol: SymbolHandle,
    ) -> Checked<TypeParameter> {
        if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(super::services::unavailable(
                id,
                "callable refinement capture has a stale declaration symbol",
            ));
        }
        if let Some(parameter) = header
            .generics
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            return Ok(parameter.clone());
        }
        let typed = self.typed;
        let parameter = typed.data_type_parameter_by_symbol(symbol).ok_or_else(|| {
            super::services::unavailable(
                id,
                "callable refinement capture has no exact static declaration",
            )
        })?;
        let parent = self.typed.symbols.get(symbol).parent;
        let mut owner = header.lexical_owner;
        let mut visited = Vec::new();
        while owner.is_valid() && !visited.contains(&owner) {
            if owner == parent {
                return Ok(parameter.clone());
            }
            visited.push(owner);
            owner = self.typed.symbols.get(owner).parent;
        }
        Err(super::services::unavailable(
            id,
            "callable refinement capture is outside its retained lexical owner",
        ))
    }

    pub(super) fn refinement_scalar_type(
        &self,
        id: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<ScalarType> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(super::services::unavailable(
                id,
                "callable value binder has a stale carrier",
            ));
        }
        if let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            self.typed.type_reference_table.type_reference(reference)
            && let Some((_, value)) = self.scope.iter().find(|(parameter, _)| parameter == symbol)
        {
            return match value {
                ValueType::Scalar(scalar) => Ok(*scalar),
                _ => Err(super::services::unavailable(
                    id,
                    "callable value binder needs its exact closed scalar carrier",
                )),
            };
        }
        self.scalar_type(id, reference)
    }

    fn refinement_constant(
        &self,
        id: MachineId,
        header: &Header<'t>,
        symbol: SymbolHandle,
        carrier: TypeReferenceHandle,
        identity: &language_semantics::const_value::CanonicalConstIdentity,
    ) -> Checked<semantic_vocabulary::ScalarTerm> {
        let parameter = self.refinement_parameter(id, header, symbol)?;
        let declared = match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => type_reference,
            _ => {
                return Err(super::services::unavailable(
                    id,
                    "callable constant operand belongs to a different binder category",
                ));
            }
        };
        let scalar = self.refinement_scalar_type(id, carrier)?;
        if self.refinement_scalar_type(id, declared)? != scalar {
            return Err(super::services::unavailable(
                id,
                "callable constant operand changed its declared carrier",
            ));
        }
        canonical_scalar_constant(id, scalar, identity)
    }

    fn refinement_header(
        &self,
        id: MachineId,
        source: CallableDeclaration<'t>,
        lexical_owner: SymbolHandle,
    ) -> Checked<Header<'t>> {
        let typed = self.typed;
        let mut header = match source {
            CallableDeclaration::Signature(signature) => {
                if signature.spelling.is_some()
                    || signature.is_default
                    || !signature.native_callback_parameters.is_empty()
                    || !signature.conformance_bounds.is_empty()
                {
                    return Err(super::services::unavailable(
                        id,
                        "callable refinement signature has unrepresented operator, default, callback or evidence metadata",
                    ));
                }
                Header {
                    declaration: signature.symbol,
                    owner: signature.symbol,
                    lexical_owner: signature.symbol,
                    name: signature.name.to_string(),
                    generics: typed.state_signature_type_parameters(signature).to_vec(),
                    parameters: super::span(
                        &typed.state_parameters,
                        signature.parameters,
                        id,
                        "callable parameter span",
                    )?,
                    result: signature.return_type,
                    contracts: super::span(
                        &typed.signature_contracts,
                        signature.contracts,
                        id,
                        "callable clause span",
                    )?
                    .to_vec(),
                    declared: Declarer::Requirement {
                        declares_no_crash_route: signature.declares_no_crash_route,
                    },
                }
            }
            CallableDeclaration::Operator(operator) => Header {
                declaration: operator.symbol,
                owner: operator.symbol,
                lexical_owner: operator.symbol,
                name: typed.operator_path_string(operator.name),
                generics: typed.operator_type_parameters(operator).to_vec(),
                parameters: super::span(
                    &typed.state_parameters,
                    operator.parameters,
                    id,
                    "operator parameter span",
                )?,
                result: operator.return_type,
                contracts: super::span(
                    &typed.signature_contracts,
                    operator.contracts,
                    id,
                    "operator clause span",
                )?
                .to_vec(),
                declared: Declarer::Operator,
            },
            CallableDeclaration::Machine(symbol) => {
                let (machine, state) = self.declaration(id, symbol)?;
                self.machine_refinement_header(id, machine, state)?
            }
            CallableDeclaration::MachineState { declaration, entry } => {
                let machine = typed.machine_by_symbol(declaration).ok_or_else(|| {
                    super::services::unavailable(
                        id,
                        "callable refinement lost its selected machine declaration",
                    )
                })?;
                let state = typed.state_in_machine(machine, entry).ok_or_else(|| {
                    super::services::unavailable(
                        id,
                        "callable refinement lost its selected entry declaration",
                    )
                })?;
                self.machine_refinement_header(id, machine, state)?
            }
        };
        if !typed.symbols.symbols().nodes().is_valid(header.declaration)
            || (lexical_owner.is_valid()
                && !typed.symbols.symbols().nodes().is_valid(lexical_owner))
        {
            return Err(super::services::unavailable(
                id,
                "callable refinement lost its declaration or lexical owner",
            ));
        }
        if lexical_owner.is_valid() {
            header.lexical_owner = lexical_owner;
        }
        Ok(header)
    }

    fn machine_refinement_header(
        &self,
        id: MachineId,
        machine: &'t typed_trees::machine::Machine,
        state: &'t typed_trees::state::State,
    ) -> Checked<Header<'t>> {
        let typed = self.typed;
        let mut contracts = super::span(
            &typed.signature_contracts,
            machine.contracts,
            id,
            "machine clause span",
        )?
        .to_vec();
        contracts.extend_from_slice(super::span(
            &typed.signature_contracts,
            state.contracts,
            id,
            "entry clause span",
        )?);
        Ok(Header {
            declaration: machine.symbol,
            owner: state.symbol,
            lexical_owner: machine.symbol,
            name: machine.name.to_string(),
            generics: typed.machine_callable_parameters(machine),
            parameters: super::span(
                &typed.state_parameters,
                state.parameters,
                id,
                "machine parameter span",
            )?,
            result: state.return_type,
            contracts,
            declared: Declarer::Machine {
                declares_no_crash_route: machine.declares_no_crash_route,
                is_public: machine.is_public,
            },
        })
    }

    fn refinement_row(
        &mut self,
        header: Header<'t>,
        binders: &[ValueBinder],
        bound: &[(SymbolHandle, semantic_vocabulary::ScalarTerm)],
    ) -> Checked<MachineId> {
        let id = self.next_identity()?;
        let previous_owner = std::mem::replace(&mut self.lifetime_owner, header.lexical_owner);
        let lowered = self.contract_parts(
            id,
            header.owner,
            header.parameters,
            header.result,
            &header.contracts,
            header.declared,
            binders,
            bound,
        );
        self.lifetime_owner = previous_owner;
        let (mut signature, mut names) = lowered?;
        // Operational guarantees are checked by their own exact header owner;
        // this abstract invocation proves logical clauses only.
        signature.termination = Termination::Silent;
        names.machine = header.name;
        self.symbols.push(header.declaration);
        self.signatures.push(signature);
        self.names.push(names);
        self.value_parameters.push(binders.len());
        self.value_arguments.push(Vec::new());
        self.scopes.push(self.scope.clone());
        Ok(id)
    }
}

/// Decode one canonical scalar value against its resolved semantic carrier.
/// Type applications use the same rule as closed source scalar declarations.
pub(super) fn canonical_scalar_constant(
    id: MachineId,
    scalar: ScalarType,
    identity: &language_semantics::const_value::CanonicalConstIdentity,
) -> Checked<semantic_vocabulary::ScalarTerm> {
    use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
    use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarTerm};
    let unavailable = |detail| super::services::unavailable(id, detail);
    let decoded = identity
        .decode_encoding()
        .ok_or_else(|| unavailable("source scalar operand has a malformed canonical encoding"))?;
    match (scalar, decoded) {
        (ScalarType::Boolean, DecodedCanonicalConstValue::Boolean(value))
            if *identity
                == language_semantics::const_value::CanonicalConstValue::boolean(value)
                    .identity() =>
        {
            Ok(ScalarTerm::Boolean(value))
        }
        (
            ScalarType::Integer(integer),
            DecodedCanonicalConstValue::Integer { type_name, value },
        ) => {
            if integer.is_address() {
                return Err(unavailable(
                    "source address constant needs its target-dependent carrier",
                ));
            }
            // Canonical integer encodings name their builtin carrier by sign
            // and width. The declared type was resolved by symbol before here.
            let name = format!(
                "{}{}",
                if integer.sign() == IntegerSign::Signed {
                    "i"
                } else {
                    "u"
                },
                integer.bits()
            );
            if type_name != name || *identity != CanonicalConstIdentity::integer(type_name, value) {
                return Err(unavailable(
                    "source scalar operand encoding names another carrier",
                ));
            }
            let value = match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(value),
                IntegerSign::Unsigned => IntegerValue::Unsigned(
                    u128::try_from(value)
                        .map_err(|_| unavailable("source unsigned scalar operand is negative"))?,
                ),
            };
            if !integer.admits(value) {
                return Err(unavailable(
                    "source scalar operand is outside its exact carrier",
                ));
            }
            Ok(ScalarTerm::Integer {
                scalar_type: integer,
                value,
            })
        }
        _ => Err(unavailable(
            "source operand needs a represented mathematical scalar value",
        )),
    }
}
