/*
These queries read retained declaration identities and already formed types.
They establish no contract, value inhabitation, or body judgment. A missing
required type is unavailable rather than reconstructed with the retired checker.
*/
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::FloatFormat;
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    data::DataField,
    expression::{ExpressionHandle, ExpressionNode, TableNamePath, TableStructLiteral},
    signature::StateParameter,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) fn float_literal_atom(format: FloatFormat) -> BuiltinTypeAtom {
    match format {
        FloatFormat::F32 => BuiltinTypeAtom::F32,
        FloatFormat::F64 => BuiltinTypeAtom::F64,
    }
}

pub(crate) fn exact_declared_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataField> {
    program.unique_declared_field_by_symbol(symbol)
}

/// The parameters a call of `target` passes its arguments to, or `None` when
/// typing does not select them: a machine with no state, or a name that no
/// state, signature or operator answers alone. The caller records that on the
/// declaration holding the call rather than refusing the program.
pub(crate) fn call_target_parameters(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Option<&[StateParameter]> {
    if let Some(state) = program.state_by_symbol(target) {
        return Some(program.state_parameters(state));
    }
    if let Some(machine) = program.machine_by_symbol(target) {
        return program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        return Some(program.state_signature_parameters(signature));
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        return Some(program.operator_parameters(operator));
    }
    let mut signatures = program
        .traits()
        .iter()
        .flat_map(|declaration| program.trait_machine_signatures(declaration))
        .filter(|signature| signature.symbol == target);
    if let Some(signature) = signatures.next()
        && signatures.next().is_none()
    {
        return Some(program.state_signature_parameters(signature));
    }
    None
}

pub(crate) fn selected_construction_field_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let table = &program.expression_table;
    let application = [
        table.construction_required_type_reference(expression),
        table.constructed_value_type_reference(expression),
        literal.application_type,
    ]
    .into_iter()
    .find(|reference| reference.is_valid());
    if let Some(application) = application {
        if let Some(field) =
            program.data_constructor_field(application, literal.type_symbol, symbol)
        {
            let parent = program.symbols.get(field.symbol).parent;
            if program.symbols.get(parent).kind == SymbolKind::Variant {
                let case = literal.case_symbol.and_then(|case| {
                    program.data_constructor_case(application, literal.type_symbol, case)
                });
                if case.is_none_or(|case| case.symbol != parent) {
                    return Err(crate::unavailable_judgment(
                        "selected constructor case field type",
                    ));
                }
            }
            return Ok(field.type_reference);
        }
        return Err(crate::unavailable_judgment(
            "selected constructor field type",
        ));
    }
    let definition = program
        .unique_data_definition_by_symbol(literal.type_symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor declaration types"))?;
    if !program.data_type_parameters(definition).is_empty() || definition.generic_instance.is_some()
    {
        return Err(crate::unavailable_judgment(
            "applied constructor field type",
        ));
    }
    let field = exact_declared_field(program, symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor field type"))?;
    let parent = program.symbols.get(symbol).parent;
    let owner = if program.symbols.get(parent).kind == SymbolKind::Variant {
        if literal.case_symbol != Some(parent) {
            return Err(crate::unavailable_judgment("constructor case identity"));
        }
        program.symbols.get(parent).parent
    } else {
        parent
    };
    if owner != literal.type_symbol {
        return Err(crate::unavailable_judgment("constructor field owner"));
    }
    Ok(field.type_reference)
}

pub(crate) fn collection_element_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Ok(*element_type),
        _ => Err(crate::unavailable_judgment(
            "collection element type projection",
        )),
    }
}

pub(crate) fn collection_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let definition = program
        .closed_data_field_definition(reference)
        .ok_or_else(|| crate::unavailable_judgment("applied collection field type"))?;
    let field = program
        .data_constructor_field(reference, definition.symbol, symbol)
        .ok_or_else(|| crate::unavailable_judgment("collection field owner/type"))?;
    Ok(field.type_reference)
}

/// Read a storage destination's declared type so its literal can retain the
/// same applied constructor. Bounds, case selection, access and establishment
/// remain obligations of the consuming lowered operation.
pub(crate) fn declared_place_type(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    target: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    declared_projection_type(program, target, |path| {
        if path.reserved_result_of.is_valid() {
            return Err(crate::unavailable_judgment(
                "reserved result is not storage",
            ));
        }
        let binder = path
            .resolved_symbol()
            .ok_or_else(|| crate::unavailable_judgment("assignment storage binder"))?;
        program
            .declared_binding_type_by_symbol(state, binder)
            .ok_or_else(|| crate::unavailable_judgment("unique assignment storage declaration"))
    })
}

/// Read the declared type of a value in its exact callable context. A
/// contract's reserved result is a value of its declared return type, never
/// writable storage or a name resolved again by spelling.
pub(crate) fn declared_callable_value_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    target: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    if !program.symbols.symbols().nodes().is_valid(owner) {
        return Err(crate::unavailable_judgment("contract value owner"));
    }
    // Entry observation preserves the operand's complete declared type. This
    // is a value query, not a writable-place query or admission of an `old`
    // subject: the lowered reader still checks its input identity and phase.
    let table = &program.expression_table;
    if !table.expression_is_valid(target) {
        return Err(crate::unavailable_judgment("declared value expression"));
    }
    // A suffixed literal fixes its own carrier. Reading a receiving type here
    // could re-land it in a different carrier or silently drop its policy.
    let literal = if table.expression_is_valid(target) {
        match table.expression(target) {
            ExpressionNode::Integer(literal) => literal.landing().map(|landing| {
                (
                    typed_trees::types::builtin_type_atom(landing.landed_type),
                    landing.domain,
                )
            }),
            ExpressionNode::Float(literal) => literal
                .landing()
                .map(|format| (float_literal_atom(format), ArithmeticDomain::Exact)),
            ExpressionNode::Boolean(_) => Some((BuiltinTypeAtom::Bool, ArithmeticDomain::Exact)),
            _ => None,
        }
    } else {
        None
    };
    if let Some((atom, policy)) = literal {
        let symbol = program
            .symbols
            .builtin_atom_symbol(atom)
            .ok_or_else(|| crate::unavailable_judgment("literal's retained carrier"))?;
        return program
            .type_reference_table
            .find_arithmetic_result_type_reference(symbol, policy)
            .ok_or_else(|| crate::unavailable_judgment("literal's retained qualified type"));
    }
    let mut target = target;
    for _ in 0..table.expression_count() {
        if !table.expression_is_valid(target) {
            break;
        }
        let typed_trees::expression::ExpressionNode::Call(call) = table.expression(target) else {
            break;
        };
        if call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(call.target_symbol)
            || program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                != Some(symbols::BuiltinFunction::ContentOld)
        {
            break;
        }
        let [argument] = table.expression_handles(call.arguments) else {
            return Err(crate::unavailable_judgment("old input declared type arity"));
        };
        target = *argument;
    }
    let states = program
        .machine_by_symbol(owner)
        .map(|machine| program.machine_states(machine));
    let state = program.state_by_symbol(owner);
    let signature = program
        .trait_machine_signature(owner)
        .map(|(_, signature)| signature);
    if states.is_none() && state.is_none() && signature.is_none() {
        // A data declaration's own `requires` names its fields bare
        // (`requires (root as -Handle) != 0`), each a value of the type the
        // declaration gives it.
        let Some(definition) = program.unique_data_definition_by_symbol(owner) else {
            return Err(crate::unavailable_judgment("contract callable declaration"));
        };
        return declared_projection_type(program, target, |path| {
            if path.static_application.is_valid() || path.reserved_result_of.is_valid() {
                return Err(crate::unavailable_judgment("data clause value"));
            }
            let field = path
                .resolved_symbol()
                .and_then(|binder| program.data_field(definition, binder))
                .ok_or_else(|| crate::unavailable_judgment("data clause field"))?;
            program
                .type_reference_table
                .contains_type_reference(field.type_reference)
                .then_some(field.type_reference)
                .ok_or_else(|| crate::unavailable_judgment("data clause field declared type"))
        });
    }
    declared_projection_type(program, target, |path| {
        if path.static_application.is_valid() {
            return Err(crate::unavailable_judgment("applied contract value"));
        }
        let reference = if path.reserved_result_of.is_valid() {
            if path.resolved_symbol().is_some()
                || program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
                || program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .any(|symbol| symbol.is_valid())
            {
                return Err(crate::unavailable_judgment("reserved result identity"));
            }
            let result_owner = path.reserved_result_of;
            let mut results = states
                .into_iter()
                .flatten()
                .chain(state)
                .filter(|state| state.symbol == result_owner)
                .map(|state| state.return_type)
                .chain(
                    signature
                        .filter(|signature| signature.symbol == result_owner)
                        .map(|signature| signature.return_type),
                );
            let reference = results
                .next()
                .ok_or_else(|| crate::unavailable_judgment("result of another callable"))?;
            if results.next().is_some() {
                return Err(crate::unavailable_judgment(
                    "ambiguous contract result owner",
                ));
            }
            reference
        } else {
            let binder = path
                .resolved_symbol()
                .ok_or_else(|| crate::unavailable_judgment("contract value binder"))?;
            if !program.symbols.symbols().nodes().is_valid(binder) {
                return Err(crate::unavailable_judgment("contract value binder"));
            }
            let mut declarations = states
                .into_iter()
                .flatten()
                .chain(state)
                .filter_map(|state| program.declared_binding_type_by_symbol(state, binder))
                .chain(signature.into_iter().flat_map(|signature| {
                    program
                        .state_signature_parameters(signature)
                        .iter()
                        .filter(move |parameter| parameter.symbol == binder)
                        .map(|parameter| parameter.type_reference)
                }));
            let reference = declarations
                .next()
                .ok_or_else(|| crate::unavailable_judgment("contract value declaration"))?;
            if declarations.next().is_some() {
                return Err(crate::unavailable_judgment(
                    "ambiguous contract value declaration",
                ));
            }
            reference
        };
        program
            .type_reference_table
            .contains_type_reference(reference)
            .then_some(reference)
            .ok_or_else(|| crate::unavailable_judgment("contract value declared type"))
    })
}

fn declared_projection_type(
    program: &TypedTrees,
    target: ExpressionHandle,
    mut root_type: impl FnMut(&TableNamePath) -> Result<TypeReferenceHandle, Diagnostic>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let table = &program.expression_table;
    let mut spine = Vec::new();
    let mut cursor = target;
    // Scratch grows only with this expression spine. The arena's existing
    // count bounds malformed cycles without allocating a whole-arena bitmap.
    for _ in 0..table.expression_count() {
        if !table.expression_is_valid(cursor) {
            break;
        }
        match table.expression(cursor) {
            ExpressionNode::Name(path) => {
                let names = table.name_path_members(path.members);
                let selected = table.name_path_member_symbols(path.member_symbols);
                if names.is_empty() || (!selected.is_empty() && selected.len() != names.len()) {
                    return Err(crate::unavailable_judgment(
                        "assignment storage path roster",
                    ));
                }
                if let Some(binder) = path.resolved_symbol()
                    && selected
                        .first()
                        .is_some_and(|symbol| symbol.is_valid() && *symbol != binder)
                {
                    return Err(crate::unavailable_judgment(
                        "assignment storage binder identity",
                    ));
                }
                let mut reference = root_type(path)?;
                for (position, name) in names.iter().enumerate().skip(1) {
                    reference = declared_field_type(
                        program,
                        reference,
                        name.as_str(),
                        selected.get(position).copied().unwrap_or_default(),
                    )?;
                }
                for expression in spine.into_iter().rev() {
                    reference = match table.expression(expression) {
                        ExpressionNode::Member(member) if member.case_variant.is_none() => {
                            declared_field_type(
                                program,
                                reference,
                                member.member.as_str(),
                                member.selection.field_symbol(),
                            )?
                        }
                        ExpressionNode::Indexed(indexed)
                            if table.range_node(indexed.index).is_none() =>
                        {
                            collection_element_type(program, reference)?
                        }
                        _ => {
                            return Err(crate::unavailable_judgment("assignment storage selector"));
                        }
                    };
                    if !program
                        .type_reference_table
                        .contains_type_reference(reference)
                    {
                        return Err(crate::unavailable_judgment(
                            "assignment selected type occurrence",
                        ));
                    }
                }
                return Ok(reference);
            }
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                spine.push(cursor);
                cursor = member.receiver;
            }
            ExpressionNode::Indexed(index) => {
                spine.push(cursor);
                cursor = index.collection;
            }
            _ => break,
        }
    }
    Err(crate::unavailable_judgment("assignment storage expression"))
}

fn declared_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    selected: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let carrier = program
        .type_reference_table
        .peeled_type_reference(reference);
    let data = program
        .closed_data_field_definition(carrier)
        .ok_or_else(|| crate::unavailable_judgment("assignment field application"))?;
    let mut fields = program
        .data_members(data)
        .iter()
        .filter_map(typed_trees::data::DataMember::as_field)
        .filter(|field| field.name.as_str() == name);
    let field = fields
        .next()
        .ok_or_else(|| crate::unavailable_judgment("assignment field declaration"))?;
    if fields.next().is_some() || (selected.is_valid() && selected != field.symbol) {
        return Err(crate::unavailable_judgment("assignment field identity"));
    }
    collection_field_type(program, carrier, field.symbol)
}

use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::types::PrimitiveType;
pub(crate) fn land_owned_fixed_byte_literal(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    let ExpressionNode::String(bytes) = program.expression_table.expression(value).clone() else {
        return Ok(());
    };
    let mut carrier = destination;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: _,
    } = program.type_reference_table.type_reference(carrier).clone()
    else {
        return Ok(());
    };

    if program.primitive_type_reference(element_type) != Some(PrimitiveType::U8) {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed array `{}`: its element type must be `u8`",
            program.display_type_reference(destination),
        )));
    }
    let Some(width) = program.closed_fixed_array_length(carrier) else {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed byte array `{}`: its width must be a compile-known resolved integer literal",
            program.display_type_reference(destination),
        )));
    };
    let actual = bytes.len();
    if actual != width {
        return Err(Diagnostic::error(format!(
            "quoted byte literal has {actual} source byte(s), but owned fixed byte array `{}` requires exactly {width}; fixed-array literal copying neither truncates nor pads",
            program.display_type_reference(destination),
        )));
    }

    let elements = bytes
        .iter()
        .map(|byte| {
            program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(i64::from(*byte)),
            ))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    *program.expression_table.expression_mut(value) = ExpressionNode::ArrayLiteral(elements);
    Ok(())
}

fn static_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Module
                | SymbolKind::BuiltinType
                | SymbolKind::Data
                | SymbolKind::Domain
                | SymbolKind::TypeAlias
                | SymbolKind::Machine
                | SymbolKind::Trait
                | SymbolKind::Conformance
        )
}

/// What a call is written on.
#[derive(Clone, Copy)]
enum CallReceiver {
    Absent,
    /// A written path. Its symbol is invalid when the path names a value
    /// rather than a declaration.
    Named(SymbolHandle),
    /// Any other expression, which is always a value.
    Value,
}

/// The declarations a call may name directly.
struct DeclaredCallTargets<'program> {
    program: &'program TypedTrees,
    // Keys of the program's mathematical definitions, sorted.
    mathematical_definitions: Vec<(u32, u32)>,
}

impl<'program> DeclaredCallTargets<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        let mut mathematical_definitions = program
            .mathematical_definitions()
            .iter()
            .map(|definition| crate::lowerer::symbol_key(definition.symbol))
            .collect::<Vec<_>>();
        mathematical_definitions.sort_unstable();
        Self {
            program,
            mathematical_definitions,
        }
    }

    /// Whether `symbol` is a declaration this call form names. Typing only
    /// says the call is bound; what the call must establish is judged where
    /// the declaration holding it is read.
    fn declares(&self, symbol: SymbolHandle, receiver: Option<SymbolHandle>) -> bool {
        let program = self.program;
        // A requirement has a declaration before it has a supplied body. Retain
        // its exact signature for lowering; availability and call obligations do
        // not become facts merely because preparation can identify the target.
        if !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
            return false;
        }
        if program.machine_by_own_or_state_symbol(symbol).is_some()
            || program.operator_definition_by_symbol(symbol).is_some()
            || program.trait_machine_signature(symbol).is_some()
        {
            return receiver.is_none_or(|receiver| static_namespace(program, receiver));
        }
        match program.symbols.get(symbol).kind {
            // mathematical_bindings.md, "Named mathematical definitions": a
            // top-level `let` with a parameter telescope is applied by name,
            // `core::Identity<T>(left, right)`. A module may qualify it; a
            // value never does.
            SymbolKind::MathematicalDefinition => {
                return receiver.is_none_or(|receiver| static_namespace(program, receiver))
                    && self
                        .mathematical_definitions
                        .binary_search(&crate::lowerer::symbol_key(symbol))
                        .is_ok();
            }
            // A compiler operation such as `embed(value)` has one bare name
            // and no qualified or receiver form.
            SymbolKind::BuiltinFunction => {
                return receiver.is_none()
                    && program
                        .symbols
                        .builtin_function_for_symbol(symbol)
                        .is_some();
            }
            // generics.md, "Callable parameter types": a static callable
            // parameter is called through its complete written callable
            // contract. One whose type is still to be inferred has no
            // contract to call through and stays a request.
            SymbolKind::MachineParameter => {
                return receiver.is_none()
                    && program.retained_static_machine_contract(symbol).is_some();
            }
            _ => {}
        }
        // An evidence child belongs to its actual bound namespace. Reuse the
        // applied-requirement query; a similarly named binder or a value receiver
        // cannot supply this declaration, and ambiguity remains unavailable.
        let binder = program.symbols.get(symbol).parent;
        binder.is_valid()
            && receiver.is_none_or(|receiver| receiver == binder)
            && matches!(
                program.evidence_bound_requirement(
                    program.symbols.get(binder).parent,
                    binder,
                    symbol,
                ),
                Ok(Some(_))
            )
    }

    /// The same checked row identifies an inline body or an existing machine.
    /// Closing its original selection records that choice, not its validity.
    fn named_call(&self, target: SymbolHandle, receiver: CallReceiver) -> bool {
        let CallReceiver::Named(namespace) = receiver else {
            return false;
        };
        matches!(
            self.program.named_conformance_member(namespace, target),
            Ok(Some(_))
        )
    }

    fn named_expression_call(&self, call: &typed_trees::expression::TableCallExpression) -> bool {
        matches!(
            self.program.named_conformance_call_member(call),
            Ok(Some(_))
        )
    }

    /// What a call typing did not bind still asks of a later pass, or `None`
    /// for a call of a declared target.
    fn unserved(&self, target: SymbolHandle, receiver: CallReceiver) -> Option<DeferredRequest> {
        let receiver = match receiver {
            CallReceiver::Absent => None,
            CallReceiver::Named(symbol) => Some(symbol),
            CallReceiver::Value => return Some(DeferredRequest::ReceiverCallSelection),
        };
        if self.declares(target, receiver) {
            return None;
        }
        Some(
            if receiver.is_some_and(|receiver| !static_namespace(self.program, receiver)) {
                DeferredRequest::ReceiverCallSelection
            } else if !target.is_valid() {
                DeferredRequest::UnresolvedCallTarget
            } else {
                DeferredRequest::UncallableCallTarget
            },
        )
    }
}

/// Closes exact named operation choices before recording calls typing left
/// without a declared target on their owning declaration. Selection closure
/// identifies a public requirement; it grants no body or contract judgment.
///
/// Typing sees the program with every bundled library and does not know what
/// an entry reaches, so such a call is not refused here: it is a row of the
/// declaration, raised by whoever demands that declaration. A call of a bare
/// name with no visible declaration is an error in every program and its row
/// says so (`DeferredRequest::UndeclaredCallName`); other unserved selections
/// stay requests. Resolution retains absence at each exact occurrence; typing
/// never asks unrelated declarations to reinterpret it. A missing value name
/// is the same error (`DeferredRequest::UndeclaredValueName`). A
/// terminal call is examined for the machines of this cohort only, as before.
/// A request in an expression no declaration owns has no reader to raise it
/// and still refuses here.
pub(crate) fn defer_unbound_calls(
    program: &mut TypedTrees,
    frontier: usize,
    completed: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    use typed_trees::statement::TransitionTargetNode;
    let mut in_expressions = Vec::new();
    // Each row retains resolution's scoped outcome until its owner is known.
    let mut in_declarations = Vec::new();
    let mut closed_selections = Vec::new();
    let rows: Vec<(SymbolHandle, DeferredRequest, source::SourceSpan)>;
    {
        let program = &*program;
        let targets = DeclaredCallTargets::new(program);
        for (handle, expression) in program.expression_table.expression_entries() {
            if let ExpressionNode::Name(path) = expression {
                if !path.symbol.is_valid()
                    && !path.reserved_result_of.is_valid()
                    && program.expression_table.unresolved_name(handle)
                        == symbols::UnresolvedName::NoVisibleDeclaration
                {
                    in_expressions.push((
                        handle,
                        DeferredRequest::UndeclaredValueName,
                        symbols::UnresolvedName::Unserved,
                    ));
                }
                continue;
            }
            let ExpressionNode::Call(call) = expression else {
                continue;
            };
            // `size_of<T>()` and `align_of<T>()` name no declaration by
            // construction: typing recognized the compiler-known query and
            // retained its operand, and Terminal Psi production reads that
            // request as a layout observation. It is not a call awaiting a
            // target.
            if call.layout_query.is_some() {
                continue;
            }
            let receiver = if !call.receiver.is_valid() {
                CallReceiver::Absent
            } else if let ExpressionNode::Name(path) =
                program.expression_table.expression(call.receiver)
            {
                CallReceiver::Named(path.symbol)
            } else {
                CallReceiver::Value
            };
            if let Some(request) = targets.unserved(call.target_symbol, receiver) {
                let request = case_call(program, request, receiver, call.target.as_str());
                // A compiler operation the call was recognized as, or a
                // selection bound some other way, is not a bare name.
                let bare = if !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
                    && call.authored_call_binding.is_none()
                    && call.quotient_operation.is_none()
                    && call.private_layout_operation.is_none()
                    && call.installed_interrupt_operation.is_none()
                    && call.layout_query.is_none()
                    && call.native_code_layout.is_none()
                {
                    program.expression_table.unresolved_name(handle)
                } else {
                    Default::default()
                };
                in_expressions.push((handle, request, bare));
            }
            if targets.named_expression_call(call)
                && let Some(occurrence) = call.authored_call_selection
            {
                closed_selections.push((occurrence, call.target_symbol));
            }
        }
        for (ordinal, machine) in program.machines().iter().enumerate() {
            let in_cohort = ordinal >= frontier || completed.contains(&machine.symbol);
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call() {
                        let receiver = if program
                            .statement_table
                            .name_path_members(call.receiver)
                            .is_empty()
                        {
                            CallReceiver::Absent
                        } else {
                            CallReceiver::Named(call.receiver_symbol)
                        };
                        if let Some(request) = targets.unserved(call.target_symbol, receiver) {
                            let request =
                                case_call(program, request, receiver, call.target.as_str());
                            let bare = if call.installed_interrupt_operation.is_none()
                                && !call.static_machine_parameter.is_valid()
                            {
                                call.unresolved_name
                            } else {
                                Default::default()
                            };
                            in_declarations.push((machine.symbol, request, call.source_span, bare));
                        }
                        if targets.named_call(call.target_symbol, receiver)
                            && let Some(occurrence) = call.authored_call_selection
                        {
                            closed_selections.push((occurrence, call.target_symbol));
                        }
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    if !in_cohort {
                        continue;
                    }
                    for target in transition.targets() {
                        let TransitionTargetNode::Named {
                            path,
                            source_span,
                            static_machine_parameter,
                            ..
                        } = program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        // Keep the exact named call for the lowered checker. It
                        // binds the arguments/result and emits every callee
                        // requirement at this terminal call. Unbound or value-based
                        // dispatch still needs its real selected correspondence.
                        let receiver = if path.receiver_symbol.is_valid() {
                            CallReceiver::Named(path.receiver_symbol)
                        } else {
                            CallReceiver::Absent
                        };
                        if let Some(request) = targets.unserved(path.symbol, receiver) {
                            let bare = if !static_machine_parameter.is_valid() {
                                path.unresolved_name
                            } else {
                                Default::default()
                            };
                            in_declarations.push((machine.symbol, request, *source_span, bare));
                        }
                    }
                }
            }
        }
        if !in_expressions.is_empty() {
            let owners = typed_trees::references::collect_program_operand_owners(program);
            for (handle, request, bare) in in_expressions {
                let source_span = program.expression_table.source_span(handle);
                let mut owned = false;
                for owner in owners
                    .expression_owners(handle)
                    .filter(|owner| owner.is_valid())
                {
                    owned = true;
                    // This exact fact selects the byte predicate even though
                    // lexical lookup found no ordinary callable for its name.
                    if bare == symbols::UnresolvedName::NoVisibleDeclaration
                        && program
                            .domain_definition(owner)
                            .and_then(|domain| {
                                typed_trees::byte_predicates::byte_domain_predicate(program, domain)
                            })
                            .is_some_and(|(predicate, _)| predicate == handle)
                    {
                        continue;
                    }
                    in_declarations.push((owner, request, source_span, bare));
                }
                if !owned {
                    return Err(Diagnostic::error(format!(
                        "no product route yet: an expression outside every declaration is one {}",
                        request.construct()
                    ))
                    .with_source_span(source_span));
                }
            }
        }
        rows = in_declarations
            .into_iter()
            .map(|(declaration, request, source_span, bare)| {
                let undeclared = matches!(
                    request,
                    DeferredRequest::UnresolvedCallTarget | DeferredRequest::ReceiverCallSelection
                ) && bare == symbols::UnresolvedName::NoVisibleDeclaration;
                let request = if undeclared {
                    DeferredRequest::UndeclaredCallName
                } else {
                    request
                };
                (declaration, request, source_span)
            })
            .collect();
    }
    if !closed_selections.is_empty() {
        use typed_trees::{
            AuthoredDeclarationSelectionKind as Kind,
            AuthoredDeclarationSelectionLateBinding as Binding,
            AuthoredDeclarationSelectionTarget as Target,
        };
        for (occurrence, target) in closed_selections {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    Diagnostic::error("named operation call lost its authored selection")
                })?;
            let span = selection.source_span();
            if selection.kind() != Kind::Call {
                return Err(
                    Diagnostic::error("named operation call has a non-call selection")
                        .with_source_span(span),
                );
            }
            match selection.target() {
                Target::Resolved(actual) if actual.selected_symbol() == target => continue,
                Target::LateBound(Binding::CheckedCall) => {}
                _ => {
                    return Err(Diagnostic::error(
                        "named operation call has contradictory selection custody",
                    )
                    .with_source_span(span));
                }
            }
            program
                .finalize_late_bound_authored_declaration_selection(
                    occurrence,
                    Binding::CheckedCall,
                    target,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "named operation call lost its closed choice: {error:?}"
                    ))
                    .with_source_span(span)
                })?;
        }
    }
    for (declaration, request, source_span) in rows {
        program.record_deferred_refusal(declaration, request, source_span);
    }
    Ok(())
}

/// A call typing left without a declared target, written `Sum::Case(..)`:
/// its qualifier is a data declaration and its name one of that
/// declaration's cases. That is an error in every program
/// (`DeferredRequest::CaseCalledAsMachine`); any other request is returned
/// as it came.
fn case_call(
    program: &TypedTrees,
    request: DeferredRequest,
    receiver: CallReceiver,
    written: &str,
) -> DeferredRequest {
    let CallReceiver::Named(owner) = receiver else {
        return request;
    };
    let names_case = matches!(
        request,
        DeferredRequest::UnresolvedCallTarget | DeferredRequest::UncallableCallTarget
    ) && owner.is_valid()
        && program.symbols.get(owner).kind == SymbolKind::Data
        && program
            .symbols
            .find_child_by_name_and_kind(owner, written, SymbolKind::Variant)
            .is_some();
    if names_case {
        DeferredRequest::CaseCalledAsMachine
    } else {
        request
    }
}

/*
    The declared type of an operand, for the forms whose type is written in a
    declaration or follows from one: a place (a parameter, an annotated local,
    a field or element of one), a cast, and builtin arithmetic over plain
    operands. `None` is "typing does not read this form", never "no domains":
    the caller defers the run. The callable-value query also retains the
    compiler's `old` projection. Exact call returns are read below; matches
    and unannotated locals are not read here. The checker of the lowered form
    types every expression and re-checks each entry against the operand it finds.
*/
pub(crate) fn declared_expression_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    DeclaredExpressionTypes::default().get(program, owner, expression)
}

/// Scratch for one immutable typing frontier. Reuse it across operator sites so
/// nested operand trees are read once per actual owner, not once per parent.
#[derive(Default)]
pub(crate) struct DeclaredExpressionTypes {
    rows: std::collections::HashMap<
        (SymbolHandle, ExpressionHandle),
        Option<TypeReferenceHandle>,
        symbols::BuildSymbolHasher,
    >,
}

impl DeclaredExpressionTypes {
    pub(crate) fn get(
        &mut self,
        program: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let key = (owner, expression);
        if let Some(result) = self.rows.get(&key) {
            return *result;
        }
        // A malformed cyclic expression yields no type; it cannot recurse into
        // invented operand information. Storage grows only with queried pairs.
        self.rows.insert(key, None);
        let result = self.read(program, owner, expression);
        self.rows.insert(key, result);
        result
    }

    fn read(
        &mut self,
        program: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let table = &program.expression_table;
        if !table.expression_is_valid(expression) {
            return None;
        }
        let retained = table.constructed_value_type_reference(expression);
        if program
            .type_reference_table
            .contains_type_reference(retained)
        {
            return Some(retained);
        }
        if let ExpressionNode::Call(call) = table.expression(expression)
            && call.selects_only_nominal_route()
            && !call.receiver.is_valid()
            && call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && program
                .symbols
                .symbols()
                .nodes()
                .is_valid(call.target_symbol)
            && program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                != Some(symbols::BuiltinFunction::ContentOld)
        {
            let result = program
                .state_by_symbol(call.target_symbol)
                .map(|state| state.return_type)
                .or_else(|| {
                    program
                        .machine_by_symbol(call.target_symbol)
                        .and_then(|machine| {
                            program
                                .machine_states(machine)
                                .first()
                                .map(|state| state.return_type)
                        })
                })
                .or_else(|| {
                    program
                        .trait_machine_signature(call.target_symbol)
                        .map(|(_, signature)| signature.return_type)
                })?;
            return program
                .type_reference_table
                .contains_type_reference(result)
                .then_some(result);
        }
        match table.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Call(_) => {
                declared_callable_value_type(program, owner, expression).ok()
            }
            // An accounted entry and a settled cast carry their result. A plain
            // carrier cast results in the carrier it names.
            ExpressionNode::Cast(cast) if cast.result_type.is_valid() => Some(cast.result_type),
            ExpressionNode::Cast(cast)
                if cast.form == language_core::CastForm::Value
                    && cast.domain == ArithmeticDomain::Exact
                    && cast.semantic_domain.is_empty()
                    && cast.target_type.is_valid() =>
            {
                Some(cast.target_type)
            }
            // Selection supplies the exact declared return type before an enclosing
            // cast is accounted. No contract or membership evidence is inferred.
            ExpressionNode::Binary(binary) if binary.operator.is_arithmetic() => {
                let mut operands = Vec::with_capacity(2);
                for operand in [binary.left, binary.right] {
                    operands.push(
                        if typed_trees::closed_numeric::has_anonymous_numeric_results(
                            program, operand,
                        ) {
                            None
                        } else {
                            Some(self.get(program, owner, operand)?)
                        },
                    );
                }
                if binary.policy == ArithmeticDomain::Exact
                    && let Some(spelling) =
                        typed_trees::operator::binary_operator_spelling(binary.operator)
                    && let Some(target) = crate::operator_calls::selection_for_operands(
                        program,
                        spelling,
                        &operands,
                        table.source_span(expression),
                    )
                    .ok()
                    .flatten()
                    && target.arguments.is_empty()
                {
                    return program
                        .machine_by_symbol(target.target)
                        .and_then(|machine| program.machine_states(machine).first())
                        .map(|state| state.return_type)
                        .filter(|reference| {
                            program
                                .type_reference_table
                                .contains_type_reference(*reference)
                        });
                }
                // Only unqualified builtin arithmetic can inherit its carrier.
                // A suffixed policy operation returns the same bare carrier; its
                // validity and arithmetic policy remain the checker's obligations.
                let mut result = None;
                for reference in operands.iter().flatten() {
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(*reference)
                    else {
                        return None;
                    };
                    let kind = program.symbols.get(*symbol).kind;
                    // A suffixed primitive fixes its result to the operand
                    // carrier even before a generic carrier is applied. This
                    // type query does not establish that its arithmetic is valid.
                    if !(kind == SymbolKind::BuiltinType
                        || (binary.policy != ArithmeticDomain::Exact
                            && kind == SymbolKind::TypeParameter))
                        || result.is_some_and(|earlier| earlier != *symbol)
                    {
                        return None;
                    }
                    result = Some(*symbol);
                }
                result.and_then(|_| operands.into_iter().flatten().next())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_constructor_fields_keep_case_and_constructor_owners() {
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Choice"));
        let foreign = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Foreign"));
        let cases = symbols.insert_children(
            owner,
            [
                (SymbolKind::Variant, SymbolNameRef::Static("First")),
                (SymbolKind::Variant, SymbolNameRef::Static("Second")),
            ],
        );
        let field = symbols
            .insert_children(
                cases.start(),
                [(SymbolKind::Field, SymbolNameRef::Static("value"))],
            )
            .start();
        let foreign_field = symbols
            .insert_children(
                foreign,
                [(SymbolKind::Field, SymbolNameRef::Static("value"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let payload = program.data_payload_fields.insert_many([DataField {
            symbol: field,
            type_reference: reference,
            ..Default::default()
        }]);
        let members = program.data_members.insert_many([
            DataMember::Variant(DataVariant {
                symbol: cases.start(),
                payload,
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                symbol: cases.handle_at(1),
                ..Default::default()
            }),
        ]);
        program.push_data_definition(DataDefinition {
            symbol: owner,
            members,
            ..Default::default()
        });
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: foreign_field,
                type_reference: reference,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: foreign,
            members,
            ..Default::default()
        });
        let mut literal = TableStructLiteral {
            type_symbol: owner,
            case_symbol: Some(cases.start()),
            ..Default::default()
        };
        let expression = program
            .expression_table
            .insert(ExpressionNode::StructLiteral(literal.clone()));
        assert_eq!(
            selected_construction_field_type(&program, expression, &literal, field).unwrap(),
            reference
        );
        assert!(
            selected_construction_field_type(&program, expression, &literal, foreign_field)
                .unwrap_err()
                .message
                .contains("constructor field owner")
        );
        literal.case_symbol = Some(cases.handle_at(1));
        assert!(
            selected_construction_field_type(&program, expression, &literal, field)
                .unwrap_err()
                .message
                .contains("constructor case identity")
        );
        literal.case_symbol = Some(cases.start());
        let snapshot = program.clone();
        program.data_payload_fields.get_mut(payload.start()).symbol = foreign_field;
        assert!(exact_declared_field(&program, field).is_none());
        assert!(exact_declared_field(&program, foreign_field).is_none());
        assert_eq!(
            selected_construction_field_type(&snapshot, expression, &literal, field).unwrap(),
            reference
        );
    }

    #[test]
    fn assignment_places_keep_applied_fields_and_refuse_foreign_or_duplicate_binders() {
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            data::{DataDefinition, DataField, DataMember, TypeParameter},
            expression::{TableIndexedExpression, TableMemberExpression, TableNamePath},
            machine::Machine,
            name::Identifier,
            state::State,
            types::FixedArrayLength,
        };
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let owners = builder.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Cell")),
                (SymbolKind::Data, SymbolNameRef::Static("OtherCell")),
                (SymbolKind::Data, SymbolNameRef::Static("Row")),
                (SymbolKind::Machine, SymbolNameRef::Static("caller")),
            ],
        );
        let [cell, other, row, caller] = [0, 1, 2, 3].map(|index| owners.handle_at(index));
        let row_members = builder.insert_children(
            row,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Field, SymbolNameRef::Static("items")),
            ],
        );
        let foreign_field = builder
            .insert_children(other, [(SymbolKind::Field, SymbolNameRef::Static("items"))])
            .start();
        let state = builder
            .insert_children(
                caller,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .start();
        let bindings = builder.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
            ],
        );
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut named = |symbol, name| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        };
        let cell_type = named(cell, "Cell");
        let other_type = named(other, "OtherCell");
        let parameter_type = named(row_members.handle_at(0), "T");
        program.push_data_definition(DataDefinition {
            symbol: cell,
            name: Identifier::generated("Cell"),
            ..Default::default()
        });
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: foreign_field,
                name: Identifier::generated("items"),
                type_reference: cell_type,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: other,
            name: Identifier::generated("OtherCell"),
            members,
            ..Default::default()
        });
        let element_array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: parameter_type,
                length: FixedArrayLength::Literal(2),
            });
        let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: row_members.handle_at(0),
            name: Identifier::generated("T"),
            ..Default::default()
        }]);
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: row_members.handle_at(1),
                name: Identifier::generated("items"),
                type_reference: element_array,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: row,
            name: Identifier::generated("Row"),
            type_parameters,
            members,
            ..Default::default()
        });
        let applications = [cell_type, other_type].map(|argument| {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles([argument]);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: row,
                    base_name: Identifier::generated("Row"),
                    arguments,
                    lifetime_arguments: Vec::new(),
                })
        });
        program
            .prepare_data_application_views(0)
            .expect("actual closed applications");
        let parameters = program
            .state_parameters
            .insert_many([0, 1].map(|index| StateParameter {
                symbol: bindings.handle_at(index),
                name: Identifier::generated(if index == 0 { "left" } else { "right" }),
                type_reference: applications[index as usize],
                ..Default::default()
            }));
        let states = program.machine_states.insert_many([State {
            symbol: state,
            parameters,
            ..Default::default()
        }]);
        program.push_machine(Machine {
            symbol: caller,
            states,
            ..Default::default()
        });
        let mut targets = Vec::new();
        let mut members = Vec::new();
        for index in [0, 1] {
            let mut names = Default::default();
            program.expression_table.push_name_path_member(
                &mut names,
                Identifier::generated(if index == 0 { "left" } else { "right" }),
            );
            let binder = bindings.handle_at(index);
            let receiver = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: names,
                    head_symbol: binder,
                    symbol: binder,
                    ..Default::default()
                }));
            let member =
                program
                    .expression_table
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver,
                        selection: typed_trees::expression::MemberSelection::Field(
                            row_members.handle_at(1),
                        ),
                        member: Identifier::generated("items"),
                        case_variant: None,
                    }));
            let selector = program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(1),
            ));
            targets.push(program.expression_table.insert(ExpressionNode::Indexed(
                TableIndexedExpression {
                    collection: member,
                    index: selector,
                },
            )));
            members.push(member);
        }
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap(),
            cell_type
        );
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[1]
            )
            .unwrap(),
            other_type
        );
        let mut foreign_state = program.machine_states(&program.machines()[0])[0].clone();
        foreign_state.parameters = Default::default();
        assert!(declared_place_type(&program, &foreign_state, targets[0]).is_err());
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(members[0])
        else {
            panic!("member")
        };
        member.selection = typed_trees::expression::MemberSelection::Field(foreign_field);
        assert!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap_err()
            .message
            .contains("assignment field identity")
        );
        member_reset(&mut program, members[0], row_members.handle_at(1));
        let applied_field = program
            .data_application_definition(applications[0])
            .expect("retained actual application")
            .members
            .start();
        let DataMember::Field(field) = program.data_members.get_mut(applied_field) else {
            panic!("field")
        };
        let original = field.type_reference;
        field.type_reference =
            TypeReferenceHandle::from_parts(original.arena_index(), original.generation() + 1);
        let error = declared_place_type(
            &program,
            &program.machine_states(&program.machines()[0])[0],
            targets[0],
        )
        .unwrap_err();
        assert_eq!(
            error.message,
            "typing could not complete the check (collection field owner/type)"
        );
        let DataMember::Field(field) = program.data_members.get_mut(applied_field) else {
            panic!("field")
        };
        field.type_reference = original;
        let array = program
            .type_reference_table
            .type_reference(original)
            .clone();
        let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = array
        else {
            panic!("array")
        };
        program.type_reference_table.substitute_node(
            original,
            TypeReferenceNode::FixedArray {
                element_type: TypeReferenceHandle::from_parts(
                    element_type.arena_index(),
                    element_type.generation() + 1,
                ),
                length: length.clone(),
            },
        );
        let error = declared_place_type(
            &program,
            &program.machine_states(&program.machines()[0])[0],
            targets[0],
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("assignment selected type occurrence"),
            "{error:?}"
        );
        program.type_reference_table.substitute_node(
            original,
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            },
        );
        let mut duplicates = program
            .state_parameters(
                program
                    .machine_states(&program.machines()[0])
                    .first()
                    .unwrap(),
            )
            .to_vec();
        duplicates.push(duplicates[0].clone());
        let parameters = program.state_parameters.insert_many(duplicates);
        let machine = program.machines()[0].clone();
        program.machine_states_mut(&machine)[0].parameters = parameters;
        assert!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap_err()
            .message
            .contains("unique assignment storage declaration")
        );
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[1]
            )
            .unwrap(),
            other_type
        );
    }

    fn member_reset(program: &mut TypedTrees, expression: ExpressionHandle, symbol: SymbolHandle) {
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            panic!("member")
        };
        member.selection = typed_trees::expression::MemberSelection::Field(symbol);
    }

    use super::*;

    /// The rows the gate records on a copy of `program`.
    fn deferred(program: &TypedTrees) -> Result<Vec<(SymbolHandle, DeferredRequest)>, Diagnostic> {
        let mut program = program.clone();
        defer_unbound_calls(&mut program, 0, &[])?;
        Ok(program
            .deferred_refusals
            .iter()
            .map(|row| (row.declaration, row.request))
            .collect())
    }

    fn call(
        receiver: ExpressionHandle,
        target_symbol: SymbolHandle,
    ) -> typed_trees::expression::TableCallExpression {
        typed_trees::expression::TableCallExpression {
            authored_call_selection: None,
            authored_call_binding: None,
            build_selection: Default::default(),
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver,
            target_symbol,
            static_machine_parameter: SymbolHandle::invalid(),
            target: typed_trees::name::Identifier::generated("call"),
            static_requirement_dispatch: None,
            machine_arguments: Box::new([]),
            quotient_operation: None,
            private_layout_operation: None,
            installed_interrupt_operation: None,
            layout_query: None,
            native_code_layout: None,
            arguments: Default::default(),
            evidence_arguments: Box::new([]),
            operational_acknowledgement: Default::default(),
        }
    }

    #[test]
    fn empty_call_and_transition_rosters_record_nothing() {
        assert_eq!(deferred(&TypedTrees::default()).unwrap(), Vec::new());
    }

    #[test]
    fn an_unbound_call_refuses_its_own_machine_and_leaves_the_others_served() {
        use typed_trees::expression::TableNamePath;
        use typed_trees::machine::{Machine, MachineBodyStatus};
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let machines = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("served"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("holder"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callee"),
                ),
            ],
        );
        let [served, holder, callee] = [0, 1, 2].map(|position| machines.handle_at(position));
        let states = [served, holder, callee].map(|machine| {
            symbols
                .insert_children(
                    machine,
                    [(SymbolKind::State, symbols::SymbolNameRef::Static("run"))],
                )
                .handle_at(0)
        });
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        // `served` calls `callee` by name; `holder` calls a method on a value
        // typing resolved to no declaration.
        let value = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::default()));
        let bound = program
            .expression_table
            .insert(ExpressionNode::Call(call(Default::default(), states[2])));
        let unbound = program
            .expression_table
            .insert(ExpressionNode::Call(call(value, SymbolHandle::invalid())));
        for ((symbol, state), body) in
            [served, holder, callee]
                .into_iter()
                .zip(states)
                .zip([Some(bound), Some(unbound), None])
        {
            let statement_nodes = program
                .statement_table
                .insert_statements(body.map(StatementNode::Expression));
            let states = program.machine_states.insert_many([State {
                symbol: state,
                statement_nodes,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::ReceiverCallSelection)]
        );
        // The same call with no receiver is a name that resolves to nothing,
        // and nothing in the program is named `call`: an error wherever it
        // is written.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.receiver = Default::default();
        program
            .expression_table
            .set_unresolved_name(unbound, symbols::UnresolvedName::NoVisibleDeclaration);
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // A spelling alone grants no compiler meaning. This synthetic table
        // has no installed declaration for the query, so it still names nothing.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = typed_trees::name::Identifier::generated("address_of");
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // A layout query typing recognized is served by the compiler and
        // records no row; the same spelling without that recognition is
        // still a name that resolves to nothing.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = typed_trees::name::Identifier::generated("size_of");
        node.layout_query = Some(typed_trees::expression::DataLayoutQueryRequest {
            kind: language_core::data_layout_queries::DataLayoutQueryKind::Size,
            operand: Default::default(),
            nonempty_view: ExpressionHandle::invalid(),
        });
        assert_eq!(deferred(&program).unwrap(), Vec::new());
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.layout_query = None;
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // An ambiguous source lookup stays unserved, regardless of which
        // declarations elsewhere happen to carry its spelling.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = typed_trees::name::Identifier::generated("run");
        program
            .expression_table
            .set_unresolved_name(unbound, symbols::UnresolvedName::Ambiguous);
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UnresolvedCallTarget)]
        );
        // The twin: once the call names a declared machine nothing is recorded.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target_symbol = states[2];
        assert_eq!(deferred(&program).unwrap(), Vec::new());
    }

    #[test]
    fn a_call_names_each_kind_of_declaration_only_in_its_own_call_form() {
        use symbols::{BuiltinFunction, builtin_function_symbols, builtin_type_symbols};
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain([
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("Identity"),
                    ),
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("undeclared"),
                    ),
                    (
                        SymbolKind::MachineParameter,
                        symbols::SymbolNameRef::Static("Step"),
                    ),
                    (SymbolKind::Module, symbols::SymbolNameRef::Static("core")),
                    (SymbolKind::Local, symbols::SymbolNameRef::Static("value")),
                ]),
        );
        let declared = symbols::BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT;
        let [identity, undeclared, step, module, value] =
            [0, 1, 2, 3, 4].map(|offset| children.handles().nth(declared + offset).unwrap());
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        program.push_mathematical_definition(typed_trees::mathematical::MathematicalDefinition {
            symbol: identity,
            ..Default::default()
        });
        let embed = program
            .symbols
            .builtin_function_symbol(BuiltinFunction::IntegerEmbed)
            .unwrap();
        let targets = DeclaredCallTargets::new(&program);
        // A mathematical definition is applied by name, bare or qualified by
        // a module, and only where the program declares it.
        assert!(targets.declares(identity, None));
        assert!(targets.declares(identity, Some(module)));
        assert!(!targets.declares(identity, Some(value)));
        assert!(!targets.declares(undeclared, None));
        // A compiler operation has only its bare name.
        assert!(targets.declares(embed, None));
        assert!(!targets.declares(embed, Some(module)));
        // A callable parameter with no written contract is not yet callable.
        assert!(!targets.declares(step, None));
        assert_eq!(
            targets.unserved(step, CallReceiver::Absent),
            Some(DeferredRequest::UncallableCallTarget)
        );
        assert_eq!(
            targets.unserved(identity, CallReceiver::Named(value)),
            Some(DeferredRequest::ReceiverCallSelection)
        );
        assert_eq!(
            targets.unserved(identity, CallReceiver::Value),
            Some(DeferredRequest::ReceiverCallSelection)
        );
        assert_eq!(targets.unserved(embed, CallReceiver::Absent), None);
    }

    #[test]
    fn an_unbound_call_outside_every_declaration_still_refuses_the_program() {
        let mut program = TypedTrees::default();
        program.expression_table.insert(ExpressionNode::Call(call(
            Default::default(),
            SymbolHandle::invalid(),
        )));
        let error = deferred(&program).unwrap_err();
        assert!(error.message.starts_with("no product route yet:"));
        assert!(error.message.contains("outside every declaration"));
    }

    #[test]
    fn a_retained_requirement_needs_its_exact_signature_and_a_namespace_receiver() {
        use typed_trees::expression::TableNamePath;
        use typed_trees::signature::StateSignature;
        use typed_trees::trait_definition::TraitDefinition;

        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let owners = symbols.insert_children(
            root,
            [
                (SymbolKind::Trait, symbols::SymbolNameRef::Static("Api")),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
            ],
        );
        let owner = owners.handle_at(0);
        let caller = owners.handle_at(1);
        let members = symbols.insert_children(
            owner,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("call")),
                (SymbolKind::State, symbols::SymbolNameRef::Static("missing")),
            ],
        );
        let requirement = members.handle_at(0);
        let missing = members.handle_at(1);
        let receiver_value = symbols
            .insert_children(
                caller,
                [(SymbolKind::Local, symbols::SymbolNameRef::Static("value"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let signatures = program
            .trait_machine_signatures
            .insert_many([StateSignature {
                symbol: requirement,
                ..Default::default()
            }]);
        program.push_trait_definition(TraitDefinition {
            symbol: owner,
            is_boundary: true,
            machines: signatures,
            ..Default::default()
        });
        let receiver = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::new(
                Default::default(),
                Default::default(),
                owner,
                owner,
            )));
        let expression = program
            .expression_table
            .insert(ExpressionNode::Call(call(receiver, requirement)));
        assert_eq!(
            deferred(&program).unwrap(),
            Vec::new(),
            "the retained requirement is a declared target"
        );
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = missing;
        assert!(deferred(&program).is_err());
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = requirement;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            panic!("receiver name")
        };
        path.symbol = receiver_value;
        path.head_symbol = receiver_value;
        assert!(deferred(&program).is_err());
    }
    #[test]
    fn bound_terminal_calls_keep_their_target_and_unbound_ones_refuse_their_machine() {
        use typed_trees::statement::{
            StatementNode, TableNamePath, TableTransition, TransitionTargetNode,
        };
        use typed_trees::{
            machine::{Machine, MachineBodyStatus},
            state::State,
        };
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let machines = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callee"),
                ),
            ],
        );
        let caller = machines.handle_at(0);
        let callee = machines.handle_at(1);
        let caller_children = symbols.insert_children(
            caller,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("entry")),
                (SymbolKind::Local, symbols::SymbolNameRef::Static("context")),
            ],
        );
        let state = caller_children.handle_at(0);
        let receiver = caller_children.handle_at(1);
        let target_state = symbols
            .insert_children(
                callee,
                [(SymbolKind::State, symbols::SymbolNameRef::Static("entry"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut members = Default::default();
        program.statement_table.push_name_path_member(
            &mut members,
            typed_trees::name::Identifier::generated("callee"),
        );
        let target =
            program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Named {
                    path: TableNamePath {
                        symbol: callee,
                        head_symbol: callee,
                        members,
                        ..Default::default()
                    },
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    source_span: Default::default(),
                    authored_call_selection: None,
                    operational_acknowledgement: Default::default(),
                    static_machine_parameter: Default::default(),
                });
        let statements = program
            .statement_table
            .insert_statements([StatementNode::Transition(TableTransition {
                target,
                ..Default::default()
            })]);
        let states = program.machine_states.insert_many([State {
            symbol: state,
            statement_nodes: statements,
            ..Default::default()
        }]);
        let target_states = program.machine_states.insert_many([State {
            symbol: target_state,
            ..Default::default()
        }]);
        for (symbol, states) in [(caller, states), (callee, target_states)] {
            program.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        assert_eq!(
            deferred(&program).unwrap(),
            Vec::new(),
            "an exact declared terminal call is retained for judgment"
        );
        let mut unbound = program.clone();
        let TransitionTargetNode::Named { path, .. } =
            unbound.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.symbol = SymbolHandle::invalid();
        assert_eq!(
            deferred(&unbound).unwrap(),
            vec![(caller, DeferredRequest::UnresolvedCallTarget)]
        );
        // A machine typed before this cohort is not examined again.
        let mut earlier = unbound.clone();
        defer_unbound_calls(&mut earlier, 2, &[]).unwrap();
        assert!(earlier.deferred_refusals.is_empty());
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.receiver_symbol = receiver;
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(caller, DeferredRequest::ReceiverCallSelection)]
        );
    }
}
