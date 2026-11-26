//! Bind a single static argument tuple against its declared parameters. Source
//! calls and Build operands use the same typed identities and requirement headers.
use crate::data::TypeParameterKind;
use crate::expression::{ExpressionNode, StaticMachineArgument};
use crate::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use crate::{TypedTrees, data_applications};
use std::borrow::Cow;
use symbols::{SymbolHandle, SymbolKind};

mod nominal_callback;
mod requirement;
use nominal_callback::require_nominal_selection;
pub use requirement::{prepare_requirement_application, prepare_requirement_in_scope};

/// Why one application was left as typing wrote it.
#[derive(Debug)]
pub enum Stop {
    /// A request this binding does not serve yet, with the check that stopped it.
    /// Owned helper errors must survive until the demanded holder reports them.
    Unserved(Cow<'static, str>),
    /// The application is wrong as the author wrote it.
    Rejected(String),
    /// A retained typed operation broke an invariant.
    Defect(&'static str),
}

pub type Served<T> = Result<T, Stop>;

pub fn unserved(reason: impl Into<Cow<'static, str>>) -> Stop {
    Stop::Unserved(reason.into())
}

/// What a static argument is, as the identity an application is keyed by:
/// two applications of one machine are the same application when each
/// parameter is given the same declaration or the same value.
#[derive(Clone, PartialEq, Eq)]
pub enum Identity {
    Type(crate::source_state::type_identity::TypeIdentityId),
    /// The declaration the argument names: a type, an implementation, a
    /// callable, or a parameter of the enclosing generic machine.
    Declaration(SymbolHandle),
    /// A closed scalar value, by its canonical spelling. The parameter it is
    /// given to fixes its type.
    Scalar(String),
    /// A structural static value, keyed by its complete canonical encoding.
    Value(String),
}

/// Applications are indexed in sorted order by template and operands. The
/// order only has to agree with equality: same variant and same payload.
impl Ord for Identity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Type(left), Self::Type(right)) => left.cmp(right),
            (Self::Declaration(left), Self::Declaration(right)) => {
                (left.arena_index(), left.generation()).cmp(&(right.arena_index(), right.generation()))
            }
            (Self::Scalar(left), Self::Scalar(right)) | (Self::Value(left), Self::Value(right)) => {
                left.cmp(right)
            }
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for Identity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Identity {
    fn rank(&self) -> u8 {
        match self {
            Self::Type(_) => 0,
            Self::Declaration(_) => 1,
            Self::Scalar(_) => 2,
            Self::Value(_) => 3,
        }
    }
}

/// One static argument read for the parameter it is given to. Every kind of
/// parameter is bound through this one shape; a kind with no reading yet is
/// unserved.
pub struct Operand {
    pub parameter: SymbolHandle,
    /// What an application is keyed by and what is substituted for the
    /// parameter.
    pub identity: Identity,
    /// The argument as the author wrote it, which replaces the parameter
    /// where a copied body passes it on as a static argument.
    pub argument: StaticMachineArgument,
    pub value: StaticMachineBindingValue,
    /// A value parameter's closed value, an integer landed in the
    /// parameter's type or a Boolean: what replaces the parameter where a
    /// copied expression names it.
    pub scalar: Option<Value>,
}

impl Operand {
    pub fn read(
        program: &mut TypedTrees,
        binder: &crate::source_state::data::TypeParameter,
        argument: &StaticMachineArgument,
        supplying_owner: SymbolHandle,
    ) -> Served<Self> {
        use crate::source_state::data::TypeParameterKind;
        let parameter = binder.symbol;
        let kind = &binder.kind;
        if !matches!(kind, TypeParameterKind::Type)
            && binder.bounds != crate::source_state::data::DataProperties::default()
        {
            return Err(unserved(
                "a non-type binder carries a property bound no application judges",
            ));
        }
        let mut scalar = None;
        let (identity, value) = match kind {
            TypeParameterKind::Type => {
                let mut reference = named_type(program, argument, supplying_owner)?;
                type_argument_meets_bounds(program, binder, reference)?;
                let identity = if matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
                ) && argument.symbol.is_valid()
                    && program.symbols.get(argument.symbol).kind == SymbolKind::TypeParameter
                {
                    Identity::Declaration(argument.symbol)
                } else {
                    let request = crate::type_identity::TypeIdentityRequest::ordinary(reference);
                    if let Some(identity) = program.region_free_scoped_type_identity(request, &[]) {
                        Identity::Type(identity)
                    } else {
                        let identity = program
                            .scoped_type_identity_in_region_scope(request, &[], supplying_owner)
                            .ok_or_else(|| unserved("static type operand lost its supplying region"))?;
                        // The copied signature reads its actual outside the caller.
                        // Retain the supplying scope on that type occurrence, too.
                        reference = program
                            .static_operand_type_reference_in_scope(argument, supplying_owner)
                            .map_err(unserved)?;
                        Identity::Type(identity)
                    }
                };
                (identity, StaticMachineBindingValue::Type(reference))
            }
            TypeParameterKind::Implementation => (
                Identity::Declaration(named_implementation(program, parameter, argument)?),
                StaticMachineBindingValue::Conformance(argument.clone()),
            ),
            TypeParameterKind::Const { type_reference }
                if program
                    .static_record_schema(*type_reference, program.symbols.get(parameter).parent)
                    .is_some() =>
            {
                let selected = named_static_record(program, parameter, *type_reference, argument)?;
                let reference = program
                    .static_operand_type_reference(argument)
                    .map_err(unserved)?;
                let identity = if program.symbols.get(selected).kind == SymbolKind::TypeParameter {
                    Identity::Declaration(selected)
                } else {
                    let sources = source_admission::source_identity::toolchain_source_identities(
                        program.symbols.source_files(),
                    )
                    .map_err(|_| unserved("a static record lost its exact source registry"))?;
                    let value = program
                        .canonical_operation_record_value(selected, &sources)
                        .map_err(unserved)?;
                    Identity::Value(value.index_identity().ok_or(unserved(
                        "a static record lost its canonical value identity",
                    ))?)
                };
                (identity, StaticMachineBindingValue::Const(reference))
            }
            TypeParameterKind::Const { type_reference }
                if program.primitive_type_reference(*type_reference).is_none()
                    && argument.symbol.is_valid()
                    && program
                        .const_declaration_by_symbol(argument.symbol)
                        .is_some() =>
            {
                let value = program
                    .canonical_aggregate_argument(*type_reference, argument)
                    .map_err(unserved)?;
                let reference = program
                    .static_operand_type_reference(argument)
                    .map_err(unserved)?;
                (
                    Identity::Value(value.index_identity().ok_or(unserved(
                        "a static aggregate lost its canonical value identity",
                    ))?),
                    StaticMachineBindingValue::Const(reference),
                )
            }
            // A value binder also admits a static argument, which selects its
            // specialization like a `const` one (generics.md, "Value binders
            // and const requirements"). A runtime argument has no static value
            // and stays unserved in `closed_scalar`.
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                let identity = match closed_scalar(program, *type_reference, argument)? {
                    Scalar::Closed(value) => {
                        let identity = Identity::Scalar(value.spelled());
                        scalar = Some(value);
                        identity
                    }
                    // A `const` parameter needs a value that is static in
                    // the caller's own body. A caller's value binder may be
                    // runtime there, so it does not supply one ("A static-only
                    // use of a dynamic binder rejects").
                    Scalar::Parameter(symbol)
                        if matches!(kind, TypeParameterKind::Const { .. })
                            && program.data_type_parameter_by_symbol(symbol).is_some_and(
                                |forwarded| {
                                    matches!(forwarded.kind, TypeParameterKind::Value { .. })
                                },
                            ) =>
                    {
                        return Err(Stop::Rejected(format!(
                            "`const` parameter `{}` needs a static value, and the value binder \
                             `{}` may be a runtime value",
                            program.symbols.name(parameter),
                            program.symbols.name(symbol),
                        )));
                    }
                    Scalar::Parameter(symbol) => Identity::Declaration(symbol),
                };
                let reference = if let Some(value) = &scalar {
                    let expression = program.expression_table.insert(value.expression());
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(expression))
                } else {
                    program
                        .static_operand_type_reference(argument)
                        .map_err(unserved)?
                };
                (identity, StaticMachineBindingValue::Const(reference))
            }
            TypeParameterKind::Machine { contract }
                if program.machine_parameter_contract_view(contract).is_some()
                    && program
                        .machine_by_symbol(program.symbols.get(parameter).parent)
                        .is_some() =>
            {
                // A nominal contract is judged by `require_nominal_callbacks`
                // once the whole application's type arguments are known.
                let chosen = named_callable(program, argument)?;
                (
                    Identity::Declaration(chosen),
                    StaticMachineBindingValue::Machine(argument.clone()),
                )
            }
            TypeParameterKind::Machine { .. } => {
                return Err(unserved(
                    "a static machine binder needs a complete contract on a body-bearing declaration",
                ));
            }
            TypeParameterKind::Proof { .. } => {
                return Err(unserved("a logical proof is not a specialization operand"));
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(unserved(
                    "a proposition binder has no specialization operand",
                ));
            }
        };
        Ok(Self {
            parameter,
            identity,
            argument: argument.clone(),
            value,
            scalar,
        })
    }

    /// Judge every nominal callback of one application after all of its
    /// operands are read. The receiving trait may name the callee's own type
    /// parameters, such as `for_each<Element, const Step: ElementStep<Element>::apply>`;
    /// the callback must satisfy that trait with the types this call supplies,
    /// whichever order the parameters are declared in.
    pub fn require_nominal_callbacks(
        program: &TypedTrees,
        parameters: &[crate::source_state::data::TypeParameter],
        operands: &[Operand],
        supplying_owner: SymbolHandle,
    ) -> Served<()> {
        let bindings = operands
            .iter()
            .filter_map(|operand| match &operand.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => reference
                    .is_valid()
                    .then_some((operand.parameter, *reference)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for operand in operands {
            let Some(binder) = parameters
                .iter()
                .find(|binder| binder.symbol == operand.parameter)
            else {
                return Err(Stop::Defect("an operand lost its declared parameter"));
            };
            let TypeParameterKind::Machine {
                contract:
                    crate::source_state::data::MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        requirement,
                        ..
                    },
            } = &binder.kind
            else {
                continue;
            };
            let Identity::Declaration(chosen) = operand.identity else {
                return Err(Stop::Defect(
                    "a nominal callback operand lost its selected declaration",
                ));
            };
            require_nominal_selection(
                program,
                nominal_callback::Receiving {
                    parameter: operand.parameter,
                    requirement: *requirement,
                    receiving_trait: *receiving_trait,
                    arguments: receiving_arguments,
                },
                chosen,
                supplying_owner,
                &bindings,
            )?;
        }
        Ok(())
    }

    /// The declaration this operand names, if it names one.
    pub fn declaration(&self) -> Option<SymbolHandle> {
        match &self.identity {
            Identity::Declaration(symbol) => Some(*symbol),
            Identity::Value(_) => self
                .argument
                .symbol
                .is_valid()
                .then_some(self.argument.symbol),
            Identity::Scalar(_) | Identity::Type(_) => None,
        }
    }

    /// Nested type and callable arguments stay open with their enclosing binders.
    pub fn is_open(&self, program: &TypedTrees) -> bool {
        self.argument.application.is_some()
            || !crate::source_state::domain::static_argument_is_closed(program, &self.argument)
    }

    /// How this operand is spelled in a generated machine's name.
    pub fn spelled(&self, program: &TypedTrees) -> String {
        match &self.identity {
            Identity::Declaration(symbol) => program.symbols.name(*symbol).to_owned(),
            Identity::Scalar(value) => value.clone(),
            Identity::Value(_) => "value".to_owned(),
            Identity::Type(identity) => program.type_identity_text(*identity).unwrap_or_default(),
        }
    }
}

/// A closed value of a builtin scalar type: what a value parameter is given.
#[derive(Clone)]
pub enum Value {
    /// An integer literal, landed in the parameter's type.
    Integer(numeric_literals::IntegerLiteral),
    Boolean(bool),
}

impl Value {
    /// The canonical spelling an application is keyed by. The parameter's
    /// type tells `1` from `true`.
    pub fn spelled(&self) -> String {
        match self {
            Self::Integer(literal) => literal.canonical_spelling().to_string(),
            Self::Boolean(value) => value.to_string(),
        }
    }

    pub fn expression(&self) -> ExpressionNode {
        match self {
            Self::Integer(literal) => ExpressionNode::Integer(literal.clone()),
            Self::Boolean(value) => ExpressionNode::Boolean(*value),
        }
    }
}

/// A value argument as this pass reads one.
enum Scalar {
    /// A closed value of the parameter's type.
    Closed(Value),
    /// A value parameter of the enclosing generic machine, passed on.
    Parameter(SymbolHandle),
}

/// The value a static argument gives a value parameter of type
/// `parameter_type`, when the argument is closed: one integer literal and
/// nothing else, for a parameter of a fixed-width builtin integer type;
/// `true` or `false` and nothing else, for a parameter of type `bool`; or
/// only the name of a parameter in scope. A literal that states another type
/// than the parameter's is not the parameter's value. An argument that
/// names a completed constant uses its canonical value; no machine runs here.
fn closed_scalar(
    program: &TypedTrees,
    parameter_type: TypeReferenceHandle,
    argument: &StaticMachineArgument,
) -> Served<Scalar> {
    let reason = match program.primitive_type_reference(parameter_type) {
        Some(primitive) => match program.prepare_callable_scalar_argument(primitive, argument) {
            Ok(value) => {
                return match value {
                    ExpressionNode::Integer(literal) => Ok(Scalar::Closed(Value::Integer(literal))),
                    ExpressionNode::Boolean(value) => Ok(Scalar::Closed(Value::Boolean(value))),
                    _ => Err(Stop::Defect("prepared callable scalar is not a scalar")),
                };
            }
            Err(reason) => reason,
        },
        None => "a static scalar binder has no primitive carrier",
    };
    let symbol = argument.symbol;
    if only_a_name(program, argument)
        && symbol.is_valid()
        && program.symbols.symbols().nodes().is_valid(symbol)
        && program.symbols.get(symbol).kind == SymbolKind::TypeParameter
    {
        return Ok(Scalar::Parameter(symbol));
    }
    Err(unserved(reason))
}

/// Whether a static argument is a name and nothing else, so that replacing
/// the declaration it names replaces all of it. Typing may keep the same
/// name a second time as a type; that says nothing more.
fn only_a_name(program: &TypedTrees, argument: &StaticMachineArgument) -> bool {
    let named_again = !argument.type_reference.is_valid()
        || matches!(
            program.type_reference_table.type_reference(argument.type_reference),
            TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
        );
    argument.application.is_none() && argument.evidence_projection.is_none() && named_again
}

/// Keep a named callable's exact declaration as the application key. A
/// parameter remains open until the caller supplies its own selected machine.
fn named_callable(program: &TypedTrees, argument: &StaticMachineArgument) -> Served<SymbolHandle> {
    let symbol = argument.symbol;
    if argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
        || !symbol.is_valid()
        || !program.symbols.symbols().nodes().is_valid(symbol)
        || (argument.type_reference.is_valid()
            && !matches!(program.type_reference_table.type_reference(argument.type_reference),
                TypeReferenceNode::Named { symbol: retained, .. } if *retained == symbol))
        || !matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
        )
    {
        return Err(unserved(
            "a static callable argument must retain its exact named machine",
        ));
    }
    Ok(symbol)
}

/// The implementation a static argument names when it is only a name, as
/// the declaration that stands for its value: an implementation parameter
/// in scope, or a named implementation with no parameters of its own, of
/// exactly the trait `parameter` is bound by. An applied implementation, a
/// projection and a bound applied at arguments are unserved here.
///
/// conformances.md, "Resulting-value identity": "A name or unused
/// constructor argument creates no additional identity", so two names that
/// construct one value are one argument, and the application is keyed by the
/// first declaration constructing it (`implementation_value_head`).
fn named_implementation(
    program: &TypedTrees,
    parameter: SymbolHandle,
    argument: &StaticMachineArgument,
) -> Served<SymbolHandle> {
    let symbol = argument.symbol;
    if !only_a_name(program, argument)
        || argument.const_literal.is_some()
        || !symbol.is_valid()
        || !program.symbols.symbols().nodes().is_valid(symbol)
    {
        return Err(unserved(
            "a static implementation argument must name an implementation without its own application",
        ));
    }
    match program.symbols.get(symbol).kind {
        SymbolKind::ConformanceParameter => Ok(symbol),
        SymbolKind::Conformance => {
            let owner = program.symbols.get(parameter).parent;
            let mut bounds = program
                .callable_conformance_bounds(owner)
                .unwrap_or_default()
                .iter()
                .filter(|bound| bound.binder == parameter);
            let (Some(bound), None) = (bounds.next(), bounds.next()) else {
                return Err(unserved(
                    "an implementation binder needs exactly one declared conformance bound",
                ));
            };
            let plain = program.conformance_by_symbol(symbol).is_some_and(|named| {
                named.trait_symbol == bound.trait_symbol
                    && bound.arguments.is_empty()
                    && bound.lifetime_arguments.is_empty()
                    && named.arguments.is_empty()
                    && named.trait_lifetime_arguments.is_empty()
                    && named.lifetime_parameters.is_empty()
                    && program.conformance_type_parameters(named).is_empty()
            });
            if !plain {
                return Err(unserved(
                    "a static implementation must be plain and match its binder's exact trait",
                ));
            }
            Ok(program.implementation_value_head(symbol))
        }
        _ => Err(unserved(
            "a static implementation argument names neither an implementation nor its parameter",
        )),
    }
}

fn named_static_record(
    program: &TypedTrees,
    parameter: SymbolHandle,
    declared_type: TypeReferenceHandle,
    argument: &StaticMachineArgument,
) -> Served<SymbolHandle> {
    if !only_a_name(program, argument)
        || argument.const_literal.is_some()
        || !program.symbols.symbols().nodes().is_valid(argument.symbol)
    {
        return Err(unserved(
            "a static record operand needs its exact named value",
        ));
    }
    let required = program
        .static_record_schema(declared_type, program.symbols.get(parameter).parent)
        .ok_or(unserved(
            "a static record parameter lost its exact declared schema",
        ))?;
    let symbol = argument.symbol;
    if program.symbols.get(symbol).kind == SymbolKind::TypeParameter {
        let declaration = program
            .data_type_parameter_by_symbol(symbol)
            .ok_or(unserved("a forwarded static record has no declared binder"))?;
        let crate::source_state::data::TypeParameterKind::Const { type_reference } =
            declaration.kind
        else {
            return Err(unserved("a forwarded static record is not a const value"));
        };
        let actual = program
            .static_record_schema(type_reference, program.symbols.get(symbol).parent)
            .ok_or(unserved(
                "a forwarded static record lost its declared schema",
            ))?;
        if !required.same_application(program, &actual) {
            return Err(Stop::Rejected(
                "a forwarded static record has another declared schema".into(),
            ));
        }
        return Ok(symbol);
    }
    let selected = program.conformance_by_symbol(symbol).ok_or(unserved(
        "a static record operand has no named record declaration",
    ))?;
    if !selected.lifetime_parameters.is_empty()
        || !program.conformance_type_parameters(selected).is_empty()
    {
        return Err(unserved(
            "a static record operand needs its complete closed application",
        ));
    }
    let actual = crate::source_state::trait_definition::AppliedTraitScope::from_conformance(
        program, selected,
    )
    .ok_or(unserved(
        "a static record operand lost its complete schema application",
    ))?;
    if !required.same_application(program, &actual) {
        return Err(Stop::Rejected(
            "a static record operand has another declared schema".into(),
        ));
    }
    // This nominates the exact value only. Ordinary named-record checking
    // owns the complete schema, member refinement and construction obligations.
    Ok(program.implementation_value_head(symbol))
}

/// A type binder's `[copy]` or `[linear]` property fixes the multiplicity
/// every use of the parameter receives (ownership.md, "Property
/// declarations"), so each application must supply a type with exactly that
/// multiplicity (generics.md: an application validates its complete tuple,
/// including constraints). The copied body is later checked as a closed
/// machine, but that check judges the body's uses, not the declared contract
/// callers rely on. An unbounded (affine) binder admits any type and leaves
/// the uses to that closed check.
pub fn type_argument_meets_bounds(
    program: &TypedTrees,
    binder: &crate::source_state::data::TypeParameter,
    reference: TypeReferenceHandle,
) -> Served<()> {
    use language_core::Multiplicity;
    let required = binder.bounds.multiplicity;
    if required == Multiplicity::Affine {
        return Ok(());
    }
    // A supplied exclusive reference carries one loan, even if its referent is copyable.
    let actual = program.stored_value_multiplicity(reference);
    if actual == required {
        return Ok(());
    }
    let property = |multiplicity| match multiplicity {
        Multiplicity::Unrestricted => "`[copy]`",
        Multiplicity::Linear => "`[linear]`",
        Multiplicity::Affine => "neither `[copy]` nor `[linear]`",
    };
    Err(Stop::Rejected(format!(
        "type argument `{}` for `{}` has {} but the binder requires {}",
        program.display_type_reference(reference),
        program.symbols.name(binder.symbol),
        property(actual),
        property(required),
    )))
}

/// Type arguments may contain the supplying declaration's binders. They retain
/// their complete type and scope; the application cannot generate executable
/// code until the ordinary static-argument closure check succeeds.
pub fn named_type(
    program: &mut TypedTrees,
    argument: &StaticMachineArgument,
    supplying_owner: SymbolHandle,
) -> Served<TypeReferenceHandle> {
    if argument.type_reference.is_valid() {
        let reference = argument.type_reference;
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(unserved("a static type operand lost its retained type"));
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::ConstExpression(_) => {
                return Err(unserved("a static type operand names a value"));
            }
            TypeReferenceNode::Named { symbol, .. }
                if program
                    .data_type_parameter_by_symbol(*symbol)
                    .is_some_and(|parameter| {
                        *symbol != argument.symbol || parameter.kind != TypeParameterKind::Type
                    }) =>
            {
                return Err(unserved(
                    "a static type operand changes its declared type binder",
                ));
            }
            TypeReferenceNode::Named { symbol, .. } => {
                require_written_lifetime_arity(program, argument, *symbol)?;
            }
            _ => {}
        }
        program
            .require_declared_application_operands(&[reference], supplying_owner)
            .map_err(unserved)?;
        let closed_view = data_applications::prepare(program, reference)
            .map_err(|reason| Stop::Rejected(format!("static type operand: {reason}")))?;
        if !closed_view && crate::source_state::domain::index_argument_is_closed(program, reference)
        {
            return Err(unserved(
                "a closed static type operand has no retained data view",
            ));
        }
        return Ok(reference);
    }
    let only_a_name = only_a_name(program, argument) && argument.const_literal.is_none();
    let symbol = argument.symbol;
    if !only_a_name || !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
        return Err(unserved(
            "a static type operand has no exact declaration name",
        ));
    }
    require_written_lifetime_arity(program, argument, symbol)?;
    let plain = match program.symbols.get(symbol).kind {
        SymbolKind::BuiltinType | SymbolKind::TypeParameter => true,
        SymbolKind::Data => program
            .unique_data_definition_by_symbol(symbol)
            .is_some_and(|data| {
                program.data_type_parameters(data).is_empty() && data.lifetime_parameters.is_empty()
            }),
        _ => false,
    };
    let Some(name) = argument.path.last().filter(|_| plain) else {
        return Err(unserved(
            "a static type operand names an unapplied data or lifetime parameter",
        ));
    };
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: name.clone(),
        });
    program
        .require_declared_application_operands(&[reference], supplying_owner)
        .map_err(unserved)?;
    Ok(reference)
}

/// Refuses a written type argument that names borrow-carrying data without its
/// lifetime arguments, such as `forward::<Loan>` for `data Loan<'lender>`.
/// lifetimes.md, "Binders and source applications": named data applications
/// "must match the declaration's lifetime arity and use binders declared in
/// the lexical owner". No rule supplies a fresh lifetime for a data type
/// argument (the per-invocation lifetimes in generics.md belong to static
/// callable requirements), so the bare name is an error in the program, not
/// a form the compiler has yet to serve.
fn require_written_lifetime_arity(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    symbol: SymbolHandle,
) -> Served<()> {
    let Some(written) = argument.path.last() else {
        return Ok(());
    };
    let Some(definition) = program.unique_data_definition_by_symbol(symbol) else {
        return Ok(());
    };
    let count = definition.lifetime_parameters.len();
    if count == 0 {
        return Ok(());
    }
    let binders = definition
        .lifetime_parameters
        .iter()
        .map(|lifetime| format!("'{}", lifetime.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let (arguments, binder) = if count == 1 {
        ("argument", "binder")
    } else {
        ("arguments", "binders")
    };
    Err(Stop::Rejected(format!(
        "type argument `{written}` supplies no lifetime arguments, but `{written}` takes \
         {count} lifetime {arguments}; write `{written}<{binders}>` with a lifetime {binder} \
         declared by the enclosing machine"
    )))
}

/// Records, on the application, which parameter each argument is for.
pub fn bind(
    program: &mut TypedTrees,
    application: StaticMachineApplicationHandle,
    operands: &[Operand],
) {
    let bindings = operands
        .iter()
        .map(|operand| StaticMachineBinding {
            parameter: operand.parameter,
            value: operand.value.clone(),
        })
        .collect::<Vec<_>>();
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
}
