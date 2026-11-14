/*
Integer carrier families: which kind of computational integer a type is, and
what a generic body may assume about a type parameter's kind.

`numeric_values.md`, "Integer carrier families": `is_integer<T>()`,
`is_signed_integer<T>()`, `is_unsigned_integer<T>()` and
`is_fixed_width_integer<T>()` are ordinary checked machines in
`core::integer`. Each states its answer in its `ensures` from one metadata
query, `integer::carrier<T>()`, whose fields this module answers from the
type's carrier identity: the sign and width of the one integer identity
(`IntegerType`, `FixedInteger`), an integer family head written alone (the
unbounded `i` and `u`), and nothing else. Every other type, nominal data with
an integer representation included, is no integer. Nothing here knows the
four query names: a call contributes a carrier fact only through its checked
`ensures result == ...` (`Program::type_truth`).

A generic check form leaves its type parameters rigid. Its own `requires`
clauses that classify a rigid parameter are its premises (`assume_premises`):
generics.md, "Integer carrier constraints", "A checked classification fact
refines the operations available for that carrier". A callee's requirement
over that parameter is then decided from those premises alone; one they do
not decide is an obligation the generic body does not establish.
*/

use semantic_vocabulary::{IntegerCarrier, IntegerSign, MachineId, ScalarType};
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use terminal_check_tables::Checked;
use terminal_check_tables::values::RigidType;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;
use crate::body::ValueType;

/// One field of `integer::Carrier`, by the name the exact toolchain
/// declaration gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Facet {
    Integer,
    Signed,
    FixedWidth,
}

impl Facet {
    fn of_field(name: &str) -> Option<Self> {
        match name {
            "integer" => Some(Self::Integer),
            "signed" => Some(Self::Signed),
            "fixed_width" => Some(Self::FixedWidth),
            _ => None,
        }
    }
}

/// What is known of one type's carrier family. A closed type knows every
/// facet; a rigid type parameter knows what its premises say, each other
/// facet `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CarrierFacts {
    integer: Option<bool>,
    signed: Option<bool>,
    fixed_width: Option<bool>,
}

impl CarrierFacts {
    /// Every type that is not a computational integer.
    const NOT_INTEGER: Self = Self {
        integer: Some(false),
        signed: Some(false),
        fixed_width: Some(false),
    };

    fn integer(sign: IntegerSign, fixed_width: bool) -> Self {
        Self {
            integer: Some(true),
            signed: Some(sign == IntegerSign::Signed),
            fixed_width: Some(fixed_width),
        }
    }

    /// The facet, with what the others imply: a signed or fixed-width
    /// carrier is an integer, and a type that is no integer is neither.
    pub(super) fn facet(self, facet: Facet) -> Option<bool> {
        match facet {
            Facet::Integer => self.integer.or(
                (self.signed == Some(true) || self.fixed_width == Some(true)).then_some(true),
            ),
            Facet::Signed if self.integer == Some(false) => Some(false),
            Facet::FixedWidth if self.integer == Some(false) => Some(false),
            Facet::Signed => self.signed,
            Facet::FixedWidth => self.fixed_width,
        }
    }

    /// Records that the facet has `truth`. `false` when it already has the
    /// other truth: the premises contradict each other.
    fn assume(&mut self, facet: Facet, truth: bool) -> bool {
        if self.facet(facet).is_some_and(|known| known != truth) {
            return false;
        }
        let slot = match facet {
            Facet::Integer => &mut self.integer,
            Facet::Signed => &mut self.signed,
            Facet::FixedWidth => &mut self.fixed_width,
        };
        *slot = Some(truth);
        true
    }
}

/// A type as a call writes it as a static argument: a type tree, or, where
/// preparation recorded only the name it resolved (a call in a contract
/// clause), that name's symbol.
#[derive(Debug, Clone, Copy)]
pub(super) enum Written {
    Reference(TypeReferenceHandle),
    Symbol(SymbolHandle),
}

/// What a type parameter stands for while a static type proposition is
/// evaluated: a value type of the machine being read (`Program::scope`), or
/// a type written at a call whose callee's contract is being evaluated.
#[derive(Debug, Clone, Copy)]
pub(super) enum Bound {
    Value(ValueType),
    Written(Written),
}

/// The type parameters a static type proposition is evaluated under: the
/// machine being read, or a callee whose contract is read at one call.
#[derive(Debug, Clone, Copy)]
pub(super) enum Scope<'s> {
    Own,
    Callee(&'s [(SymbolHandle, Bound)]),
}

impl Program<'_> {
    /// What `symbol` stands for in `scope`, if it is a type parameter there.
    pub(super) fn bound(&self, scope: Scope<'_>, symbol: SymbolHandle) -> Option<Bound> {
        match scope {
            Scope::Own => self
                .scope
                .iter()
                .find(|(binder, _)| *binder == symbol)
                .map(|(_, value_type)| Bound::Value(*value_type)),
            Scope::Callee(bindings) => bindings
                .iter()
                .find(|(binder, _)| *binder == symbol)
                .map(|(_, bound)| *bound),
        }
    }

    /// What a written type comes to under `scope`: the value type of a type
    /// parameter in scope, with any qualification of the written type left
    /// on its carrier, or the written type itself. A written type names
    /// types of its call's author, so one that is no parameter there is read
    /// where it stands, never as the callee's own parameter.
    pub(super) fn resolve_written(&self, scope: Scope<'_>, written: Written) -> Bound {
        let symbol = match written {
            Written::Symbol(symbol) => symbol,
            Written::Reference(mut handle) => {
                let types = &self.typed.type_reference_table;
                loop {
                    if !handle.is_valid() || !types.contains_type_reference(handle) {
                        return Bound::Written(Written::Reference(handle));
                    }
                    match types.type_reference(handle) {
                        TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                        TypeReferenceNode::Named { symbol, .. } => break *symbol,
                        _ => return Bound::Written(Written::Reference(handle)),
                    }
                }
            }
        };
        match self.bound(scope, symbol) {
            Some(Bound::Written(inner)) => self.resolve_written(Scope::Own, inner),
            Some(bound) => bound,
            None => Bound::Written(Written::Symbol(symbol)),
        }
    }

    /// The carrier family of a value type of the machine being read. A
    /// qualified carrier is its carrier: classifying it keeps its domains.
    pub(super) fn value_carrier(&self, value_type: ValueType) -> CarrierFacts {
        match value_type {
            ValueType::Scalar(ScalarType::Integer(integer), _)
                if integer.carrier() == IntegerCarrier::Fixed =>
            {
                CarrierFacts::integer(integer.sign(), true)
            }
            ValueType::Scalar(..) | ValueType::Record(..) => CarrierFacts::NOT_INTEGER,
            ValueType::Rigid(rigid) => self
                .rigid_carriers
                .iter()
                .find(|(known, _)| *known == rigid)
                .map(|(_, facts)| *facts)
                .unwrap_or_default(),
        }
    }

    /// The carrier family of a written type under `scope`, or `None` for a
    /// type this reader cannot resolve. A builtin integer is fixed-width at
    /// its sign; an integer family applied at any width (`u<N>`, closed or a
    /// width parameter) is too, and its head alone is the unbounded integer
    /// of its sign. Data, nominal or applied, is no integer.
    pub(super) fn written_carrier(
        &self,
        scope: Scope<'_>,
        written: Written,
    ) -> Option<CarrierFacts> {
        let symbol = match self.resolve_written(scope, written) {
            Bound::Value(value_type) => return Some(self.value_carrier(value_type)),
            Bound::Written(Written::Symbol(symbol)) => symbol,
            Bound::Written(Written::Reference(handle)) => {
                let types = &self.typed.type_reference_table;
                if !handle.is_valid() || !types.contains_type_reference(handle) {
                    return None;
                }
                return match types.type_reference(handle) {
                    TypeReferenceNode::Generic { base_symbol, .. } => {
                        if let Some(sign) = self.typed.symbols.integer_family(*base_symbol) {
                            Some(CarrierFacts::integer(integer_sign(sign), true))
                        } else {
                            self.is_data(*base_symbol)
                                .then_some(CarrierFacts::NOT_INTEGER)
                        }
                    }
                    TypeReferenceNode::Reference { .. }
                    | TypeReferenceNode::FixedArray { .. }
                    | TypeReferenceNode::Slice { .. }
                    | TypeReferenceNode::DynamicTrait { .. }
                    | TypeReferenceNode::Unit => Some(CarrierFacts::NOT_INTEGER),
                    TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Constrained { .. }
                    | TypeReferenceNode::ConstExpression(_) => None,
                };
            }
        };
        if !symbol.is_valid() {
            return None;
        }
        if let Some(atom) = self.typed.symbols.builtin_type_atom(symbol) {
            return Some(match atom {
                BuiltinTypeAtom::Integer(carrier) => {
                    CarrierFacts::integer(integer_sign(carrier.sign()), true)
                }
                _ => CarrierFacts::NOT_INTEGER,
            });
        }
        if let Some(sign) = self.typed.symbols.integer_family(symbol) {
            return Some(CarrierFacts::integer(integer_sign(sign), false));
        }
        self.is_data(symbol).then_some(CarrierFacts::NOT_INTEGER)
    }

    fn is_data(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid() && self.typed.symbols.get(symbol).kind == SymbolKind::Data
    }

    /// The callee a call names: the machine, for a call of its entry state.
    pub(super) fn called_machine(
        &self,
        call: &TableCallExpression,
    ) -> Option<&typed_trees::machine::Machine> {
        let symbols = &self.typed.symbols;
        if !call.target_symbol.is_valid() {
            return None;
        }
        let owner = match symbols.get(call.target_symbol).kind {
            SymbolKind::State => symbols.get(call.target_symbol).parent,
            _ => call.target_symbol,
        };
        self.typed.machine_by_symbol(owner)
    }

    /// Whether `call` is the carrier metadata query, by its exact toolchain
    /// declaration (`CoreVocabulary`), through a machine stage 04 generated
    /// from it as well.
    pub(super) fn is_carrier_query(&self, call: &TableCallExpression) -> bool {
        use language_core::core_vocabulary::{CoreVocabulary, IntegerCarrierDeclaration};
        self.called_machine(call).is_some_and(|machine| {
            self.typed.core_vocabulary(machine.symbol)
                == Some(CoreVocabulary::IntegerCarrier(
                    IntegerCarrierDeclaration::Query,
                ))
        })
    }

    /// The type arguments a call gives its callee, by the callee's parameter
    /// symbol: the authored application, or the one stage 04 generated the
    /// callee from. An application whose bindings preparation did not record,
    /// as in a contract clause, gives its written arguments in the callee's
    /// declaration order, one per type parameter.
    pub(super) fn call_bindings(&self, call: &TableCallExpression) -> Option<Vec<(SymbolHandle, Written)>> {
        let table = &self.typed.expression_table;
        let machine = self.called_machine(call)?;
        let application = if call.authored_machine_application.is_valid() {
            call.authored_machine_application
        } else {
            self.typed
                .machine_specialization(machine.symbol)?
                .binding_application
        };
        if !application.is_valid() {
            return None;
        }
        let bindings = table.static_machine_application_bindings(application);
        if !bindings.is_empty() {
            return bindings
                .iter()
                .map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(handle) => {
                        Some((binding.parameter, Written::Reference(handle)))
                    }
                    _ => None,
                })
                .collect();
        }
        let parameters = self.typed.machine_type_parameters(machine);
        let arguments = table.static_machine_application_arguments(application);
        if parameters.len() != arguments.len()
            || parameters
                .iter()
                .any(|parameter| parameter.kind != typed_trees::data::TypeParameterKind::Type)
        {
            return None;
        }
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                let written = if argument.type_reference.is_valid() {
                    Written::Reference(argument.type_reference)
                } else if argument.symbol.is_valid()
                    && argument.path.len() == 1
                    && argument.application.is_none()
                {
                    Written::Symbol(argument.symbol)
                } else {
                    return None;
                };
                Some((parameter.symbol, written))
            })
            .collect()
    }

    /// The carrier query a member reads and the facet it reads, when
    /// `expression` is `integer::carrier<X>().field`: `X` as written.
    fn carrier_read(&self, expression: ExpressionHandle) -> Option<(Written, Facet)> {
        let table = &self.typed.expression_table;
        if !expression.is_valid() {
            return None;
        }
        let ExpressionNode::Member(member) = table.expression(expression) else {
            return None;
        };
        if !member.receiver.is_valid() || member.case_variant.is_some() {
            return None;
        }
        let ExpressionNode::Call(call) = table.expression(member.receiver) else {
            return None;
        };
        if !self.is_carrier_query(call)
            || call.receiver.is_valid()
            || !table.expression_handles(call.arguments).is_empty()
        {
            return None;
        }
        let facet = Facet::of_field(member.member.as_str())?;
        let [(_, argument)] = self.call_bindings(call)?[..] else {
            return None;
        };
        Some((argument, facet))
    }

    /// The facet `expression` reads of the carrier query, when it is
    /// `integer::carrier<X>().field`: `Some(None)` when `X`'s family leaves
    /// that facet open, `None` when the expression is no such read.
    pub(super) fn carrier_member(
        &self,
        scope: Scope<'_>,
        expression: ExpressionHandle,
    ) -> Option<Option<bool>> {
        let (argument, facet) = self.carrier_read(expression)?;
        Some(self.written_carrier(scope, argument)?.facet(facet))
    }

    /// Takes the classifying `requires` clauses of a generic check form as
    /// premises about its rigid type parameters. Only conjunctions of facets
    /// teach anything, directly or through a callee's stated result; any
    /// other clause teaches nothing, so a requirement that needs it stays
    /// open and is not established.
    pub(super) fn assume_premises(
        &mut self,
        id: MachineId,
        clauses: &[ExpressionHandle],
    ) -> Checked<()> {
        self.rigid_carriers.clear();
        let mut taught = Vec::new();
        for clause in clauses {
            self.premise(Scope::Own, *clause, true, &mut taught);
        }
        for (rigid, facet, truth) in taught {
            let position = match self
                .rigid_carriers
                .iter()
                .position(|(known, _)| *known == rigid)
            {
                Some(position) => position,
                None => {
                    self.rigid_carriers.push((rigid, CarrierFacts::default()));
                    self.rigid_carriers.len() - 1
                }
            };
            if !self.rigid_carriers[position].1.assume(facet, truth) {
                return Err(crate::refuse(
                    id,
                    terminal_check_tables::Construct::Machine,
                    crate::Unserved::Contract("integer carrier premises that contradict each other"),
                ));
            }
        }
        Ok(())
    }

    /// The facets `expression` asserts to have `truth`, by the rigid type
    /// parameter each is about.
    fn premise(
        &self,
        scope: Scope<'_>,
        expression: ExpressionHandle,
        truth: bool,
        taught: &mut Vec<(RigidType, Facet, bool)>,
    ) {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};
        if !expression.is_valid() {
            return;
        }
        if let Some((argument, facet)) = self.carrier_read(expression) {
            if let Bound::Value(ValueType::Rigid(rigid)) = self.resolve_written(scope, argument) {
                taught.push((rigid, facet, truth));
            }
            return;
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And && truth => {
                self.premise(scope, binary.left, true, taught);
                self.premise(scope, binary.right, true, taught);
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.premise(scope, unary.operand, !truth, taught);
            }
            ExpressionNode::Call(call) if truth => {
                if let Some((bindings, stated)) = self.contract_value(scope, call) {
                    self.premise(Scope::Callee(&bindings), stated, true, taught);
                }
            }
            _ => {}
        }
    }

    /// The type arguments `call` gives its callee's parameters and the
    /// value its callee's contract says it returns, `E` of its one
    /// `ensures result == E`: a call with no value arguments, of a machine
    /// with no `requires`. `None` for any other call, which states nothing
    /// static.
    pub(super) fn contract_value(
        &self,
        scope: Scope<'_>,
        call: &TableCallExpression,
    ) -> Option<(Vec<(SymbolHandle, Bound)>, ExpressionHandle)> {
        use typed_trees::expression::BinaryOperator;
        use typed_trees::signature::SignatureContractKind;
        let typed = self.typed;
        let table = &typed.expression_table;
        if call.receiver.is_valid()
            || !table.expression_handles(call.arguments).is_empty()
            || self.is_carrier_query(call)
        {
            return None;
        }
        let machine = self.called_machine(call)?;
        let parameters = typed.machine_type_parameters(machine);
        let mut bindings = Vec::with_capacity(parameters.len());
        if !parameters.is_empty() {
            let written = self.call_bindings(call)?;
            for parameter in parameters {
                let mut bound = written
                    .iter()
                    .filter(|(binder, _)| *binder == parameter.symbol);
                let (Some((_, argument)), None) = (bound.next(), bound.next()) else {
                    return None;
                };
                bindings.push((parameter.symbol, self.resolve_written(scope, *argument)));
            }
        }
        let mut stated = None;
        for contract in typed.signature_contracts.span(machine.contracts)? {
            let facts = typed.proof_facts.span(contract.facts)?;
            match contract.kind {
                SignatureContractKind::Ensures => {}
                _ if facts.is_empty() => continue,
                _ => return None,
            }
            for fact in facts {
                let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    return None;
                };
                let ExpressionNode::Binary(binary) = table.expression(*expression) else {
                    return None;
                };
                let is_result = |handle: ExpressionHandle| {
                    handle.is_valid()
                        && matches!(
                            table.expression(handle),
                            ExpressionNode::Name(path) if path.reserved_result_of.is_valid()
                        )
                };
                let value = match binary.operator {
                    BinaryOperator::Equal if is_result(binary.left) => binary.right,
                    BinaryOperator::Equal if is_result(binary.right) => binary.left,
                    _ => return None,
                };
                if stated.replace(value).is_some() {
                    return None;
                }
            }
        }
        Some((bindings, stated?))
    }
}

fn integer_sign(sign: numerics::integer_carrier::IntegerSign) -> IntegerSign {
    match sign {
        numerics::integer_carrier::IntegerSign::Signed => IntegerSign::Signed,
        numerics::integer_carrier::IntegerSign::Unsigned => IntegerSign::Unsigned,
    }
}
