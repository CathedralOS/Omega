//! Truncating reconstruction window for `(dividend / divisor) * divisor`.
//!
//! Independent intervals forget that the product and the dividend are the
//! same value. For truncating division toward zero, a nonnegative dividend
//! and a divisor of at least one leave a remainder in `0 .. divisor`, so
//!
//! `dividend - (divisor - 1) <= (dividend / divisor) * divisor <= dividend`.
//!
//! Comparisons against that window are decided by cancelling the shared
//! linear terms, then reading only the residual against parameter bounds or
//! an unwritten field's `where` facts.
//! A negative dividend does not have this window: the product can sit
//! strictly above the dividend. Wrapping or saturating carriers are not
//! exact truncating division and stay on the interval path.

use symbols::SymbolHandle;
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    statement::StatementNode,
};

use super::super::ExitScalars;

const EXPANSION_LIMIT: usize = 8;

enum LinearShape {
    Integer(i64),
    Name(SymbolHandle),
    Member(ExpressionHandle, SymbolHandle),
    Binary(ExpressionHandle, ExpressionHandle, BinaryOperator),
    Other,
}

#[derive(Clone, Copy)]
enum ReconstructionRelation {
    AtLeast,
    AtMost,
    Above,
    Below,
    Equal,
}

/// A linear unknown. Two field spellings of the same root and member are one
/// atom even when their expression handles differ, so cancellation still sees
/// them. The retained place is only how the field's `where` interval is read.
#[derive(Clone, Copy)]
enum LinearAtom {
    Parameter(SymbolHandle),
    Field {
        root: SymbolHandle,
        field: SymbolHandle,
        place: ExpressionHandle,
    },
}

impl PartialEq for LinearAtom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Parameter(left), Self::Parameter(right)) => left == right,
            (
                Self::Field {
                    root: left_root,
                    field: left_field,
                    ..
                },
                Self::Field {
                    root: right_root,
                    field: right_field,
                    ..
                },
            ) => left_root == right_root && left_field == right_field,
            _ => false,
        }
    }
}

impl Eq for LinearAtom {}

impl LinearAtom {
    fn sort_key(self) -> (u8, u32, u32, u32, u32) {
        match self {
            Self::Parameter(symbol) => (0, symbol.arena_index(), symbol.generation(), 0, 0),
            Self::Field { root, field, .. } => (
                1,
                root.arena_index(),
                root.generation(),
                field.arena_index(),
                field.generation(),
            ),
        }
    }
}

/// Coefficients are kept in arena order so form equality does not depend on
/// the order the expression spelled its parameters.
#[derive(Clone, PartialEq, Eq)]
struct LinearForm {
    terms: Vec<(LinearAtom, i64)>,
    constant: i64,
}

impl LinearForm {
    fn constant(constant: i64) -> Self {
        Self {
            terms: Vec::new(),
            constant,
        }
    }

    fn parameter(symbol: SymbolHandle) -> Self {
        Self {
            terms: vec![(LinearAtom::Parameter(symbol), 1)],
            constant: 0,
        }
    }

    fn field(root: SymbolHandle, field: SymbolHandle, place: ExpressionHandle) -> Self {
        Self {
            terms: vec![(LinearAtom::Field { root, field, place }, 1)],
            constant: 0,
        }
    }

    fn coefficient(&self, atom: LinearAtom) -> i64 {
        self.terms
            .iter()
            .find(|(term, _)| *term == atom)
            .map(|(_, coefficient)| *coefficient)
            .unwrap_or(0)
    }

    fn set_coefficient(&mut self, atom: LinearAtom, coefficient: i64) {
        if let Some(existing) = self.terms.iter_mut().find(|(term, _)| *term == atom) {
            existing.1 = coefficient;
            if let LinearAtom::Field { place, .. } = atom {
                if let LinearAtom::Field {
                    place: existing_place,
                    ..
                } = &mut existing.0
                {
                    *existing_place = place;
                }
            }
        } else if coefficient != 0 {
            self.terms.push((atom, coefficient));
        }
        self.terms.retain(|(_, coefficient)| *coefficient != 0);
        self.terms.sort_by_key(|(atom, _)| atom.sort_key());
    }

    fn add(&self, other: &Self) -> Option<Self> {
        let mut sum = self.clone();
        sum.constant = self.constant.checked_add(other.constant)?;
        for (atom, coefficient) in &other.terms {
            let combined = self.coefficient(*atom).checked_add(*coefficient)?;
            sum.set_coefficient(*atom, combined);
        }
        Some(sum)
    }

    fn subtract(&self, other: &Self) -> Option<Self> {
        self.add(&other.scale(-1)?)
    }

    fn scale(&self, factor: i64) -> Option<Self> {
        let mut terms = Vec::new();
        for (atom, coefficient) in &self.terms {
            let scaled = coefficient.checked_mul(factor)?;
            if scaled != 0 {
                terms.push((*atom, scaled));
            }
        }
        Some(Self {
            terms,
            constant: self.constant.checked_mul(factor)?,
        })
    }

    /// Smallest value of this form over the exit state's immutable parameters.
    /// A missing endpoint that the coefficient's sign needs is not a proof.
    fn minimum(
        &self,
        scalars: &ExitScalars<'_, '_>,
        state: &typed_trees::state::State,
    ) -> Option<i64> {
        let mut minimum = self.constant;
        for (atom, coefficient) in &self.terms {
            if *coefficient == 0 {
                continue;
            }
            let (lower, upper) = match *atom {
                LinearAtom::Parameter(symbol) => validation::state_parameter_integer_interval(
                    scalars.program,
                    scalars.machine,
                    state,
                    symbol,
                )?,
                LinearAtom::Field { place, .. } => validation::stored_integer_bounds(
                    scalars.program,
                    scalars.machine,
                    state,
                    place,
                )?,
            };
            let endpoint = if *coefficient > 0 { lower } else { upper };
            let contribution = coefficient.checked_mul(endpoint?)?;
            minimum = minimum.checked_add(contribution)?;
        }
        Some(minimum)
    }
}

impl ExitScalars<'_, '_> {
    pub(super) fn truncating_reconstruction_proves(
        &self,
        operator: BinaryOperator,
        returned: ExpressionHandle,
        bound: ExpressionHandle,
        result_on_left: bool,
    ) -> bool {
        let Some(relation) = reconstruction_relation(operator, result_on_left) else {
            return false;
        };
        let Some(state) = self.state() else {
            return false;
        };
        let Some((dividend, divisor)) = self.reconstruction_forms(returned, state, &[], 0) else {
            return false;
        };
        let dividend_floor = dividend.minimum(self, state);
        let divisor_floor = divisor.minimum(self, state);
        if dividend_floor.is_none_or(|floor| floor < 0)
            || divisor_floor.is_none_or(|floor| floor < 1)
        {
            return false;
        }
        let Some(bound) = self.linear_form(bound, state, &[], &mut Vec::new(), 0) else {
            return false;
        };
        let Some(window_floor) = dividend
            .subtract(&divisor)
            .and_then(|shifted| shifted.add(&LinearForm::constant(1)))
        else {
            return false;
        };
        window_entails(relation, &window_floor, &dividend, &bound, self, state)
    }

    fn reconstruction_forms(
        &self,
        expression: ExpressionHandle,
        state: &typed_trees::state::State,
        substitution: &[(SymbolHandle, LinearForm)],
        depth: usize,
    ) -> Option<(LinearForm, LinearForm)> {
        if depth > EXPANSION_LIMIT {
            return None;
        }
        let peeled = self
            .program
            .expression_table
            .peeled_expression_handle(expression);
        let named = match self.program.expression_table.expression(peeled) {
            ExpressionNode::Name(path) => path.resolved_symbol(),
            _ => None,
        };
        if let Some(symbol) = named {
            if substitution
                .iter()
                .all(|(parameter, _)| *parameter != symbol)
            {
                if let Some(initializer) = self.local_initializer(symbol, state) {
                    return self.reconstruction_forms(initializer, state, substitution, depth + 1);
                }
            }
        }
        let call = match self.program.expression_table.expression(peeled) {
            ExpressionNode::Call(call) => Some(CopiedCall {
                target_symbol: call.target_symbol,
                receiver: call.receiver,
                arguments: call.arguments,
                blocked: call.quotient_operation.is_some()
                    || call.layout_query.is_some()
                    || call.private_layout_operation.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty(),
            }),
            _ => None,
        };
        if let Some(call) = call {
            return self.call_reconstruction(call, state, substitution, depth);
        }
        let (left, right, operator) = {
            let binary = self.program.expression_table.binary_node(peeled)?;
            (binary.left, binary.right, binary.operator)
        };
        if operator != BinaryOperator::Multiply {
            return None;
        }
        self.matched_reconstruction(left, right, state, substitution, depth)
            .or_else(|| self.matched_reconstruction(right, left, state, substitution, depth))
    }

    /// `dividend_over_divisor` is one multiply operand and `multiplier` the other.
    fn matched_reconstruction(
        &self,
        dividend_over_divisor: ExpressionHandle,
        multiplier: ExpressionHandle,
        state: &typed_trees::state::State,
        substitution: &[(SymbolHandle, LinearForm)],
        depth: usize,
    ) -> Option<(LinearForm, LinearForm)> {
        let divided = self
            .program
            .expression_table
            .peeled_expression_handle(dividend_over_divisor);
        let (dividend, divisor) = {
            let binary = self.program.expression_table.binary_node(divided)?;
            if binary.operator != BinaryOperator::Divide {
                return None;
            }
            (binary.left, binary.right)
        };
        let mut expanding = Vec::new();
        let divisor_form = self.linear_form(divisor, state, substitution, &mut expanding, depth)?;
        expanding.clear();
        let multiplier_form =
            self.linear_form(multiplier, state, substitution, &mut expanding, depth)?;
        if divisor_form != multiplier_form {
            return None;
        }
        expanding.clear();
        let dividend_form =
            self.linear_form(dividend, state, substitution, &mut expanding, depth)?;
        Some((dividend_form, divisor_form))
    }

    fn call_reconstruction(
        &self,
        call: CopiedCall,
        state: &typed_trees::state::State,
        substitution: &[(SymbolHandle, LinearForm)],
        depth: usize,
    ) -> Option<(LinearForm, LinearForm)> {
        if call.receiver.is_valid() || call.blocked {
            return None;
        }
        // A free call names the machine; an attached call names its entry
        // state. Both enter that same body.
        let entry = validation::call_target_state(self.program, call.target_symbol)?;
        if !exact_return(self, entry.return_type) {
            return None;
        }
        let parameters = self.program.state_parameters(entry);
        if parameters
            .iter()
            .any(|parameter| parameter.is_const || parameter.is_mutable)
        {
            return None;
        }
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut callee_substitution = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let mut expanding = Vec::new();
            let form = self.linear_form(*argument, state, substitution, &mut expanding, depth)?;
            callee_substitution.push((parameter.symbol, form));
        }
        let returned = callee_return_expression(self, entry)?;
        self.reconstruction_forms(returned, entry, &callee_substitution, depth + 1)
    }

    fn linear_form(
        &self,
        expression: ExpressionHandle,
        state: &typed_trees::state::State,
        substitution: &[(SymbolHandle, LinearForm)],
        expanding: &mut Vec<SymbolHandle>,
        depth: usize,
    ) -> Option<LinearForm> {
        if depth > EXPANSION_LIMIT {
            return None;
        }
        let peeled = self
            .program
            .expression_table
            .peeled_expression_handle(expression);
        let shape = match self.program.expression_table.expression(peeled) {
            ExpressionNode::Integer(literal) => LinearShape::Integer(literal.value_i64()?),
            ExpressionNode::Name(path) => LinearShape::Name(path.resolved_symbol()?),
            ExpressionNode::Member(member) => {
                LinearShape::Member(member.receiver, member.member_symbol)
            }
            ExpressionNode::Binary(binary) => {
                LinearShape::Binary(binary.left, binary.right, binary.operator)
            }
            _ => LinearShape::Other,
        };
        match shape {
            LinearShape::Integer(value) => Some(LinearForm::constant(value)),
            LinearShape::Name(symbol) => {
                if let Some((_, form)) = substitution
                    .iter()
                    .find(|(parameter, _)| *parameter == symbol)
                {
                    return Some(form.clone());
                }
                if let Some(parameter) = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                {
                    if parameter.is_const || parameter.is_mutable {
                        return None;
                    }
                    if !exact_return(self, parameter.type_reference) {
                        return None;
                    }
                    return Some(LinearForm::parameter(symbol));
                }
                self.local_form(symbol, state, substitution, expanding, depth)
            }
            LinearShape::Member(receiver, field) => self.field_form(peeled, receiver, field, state),
            LinearShape::Binary(left, right, operator) => {
                let left = self.linear_form(left, state, substitution, expanding, depth + 1)?;
                let right = self.linear_form(right, state, substitution, expanding, depth + 1)?;
                match operator {
                    BinaryOperator::Add => left.add(&right),
                    BinaryOperator::Subtract => left.subtract(&right),
                    BinaryOperator::Multiply => scale_product(&left, &right),
                    _ => None,
                }
            }
            LinearShape::Other => None,
        }
    }

    /// One field of a parameter, unread by this state. Its `where` facts are
    /// what every store preserves, so they bound the field here without
    /// replaying a mutable arrival.
    fn field_form(
        &self,
        place: ExpressionHandle,
        receiver: ExpressionHandle,
        field: SymbolHandle,
        state: &typed_trees::state::State,
    ) -> Option<LinearForm> {
        let root = self
            .program
            .expression_table
            .name_path_node(receiver)
            .and_then(|path| path.resolved_symbol())?;
        let parameter = self
            .program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == root)?;
        if parameter.is_const || self.field_is_assigned(state, root, field) {
            return None;
        }
        let (lower, upper) =
            validation::stored_integer_bounds(self.program, self.machine, state, place)?;
        if lower.is_none() && upper.is_none() {
            return None;
        }
        Some(LinearForm::field(root, field, place))
    }

    fn field_is_assigned(
        &self,
        state: &typed_trees::state::State,
        root: SymbolHandle,
        field: SymbolHandle,
    ) -> bool {
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        statements.iter().any(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return false;
            };
            let target = self
                .program
                .expression_table
                .peeled_expression_handle(assignment.target);
            let Some((member_field, member_receiver)) =
                (match self.program.expression_table.expression(target) {
                    ExpressionNode::Member(member) => Some((member.member_symbol, member.receiver)),
                    _ => None,
                })
            else {
                return false;
            };
            member_field == field
                && self
                    .program
                    .expression_table
                    .name_path_node(member_receiver)
                    .and_then(|path| path.resolved_symbol())
                    == Some(root)
        })
    }

    fn local_form(
        &self,
        symbol: SymbolHandle,
        state: &typed_trees::state::State,
        substitution: &[(SymbolHandle, LinearForm)],
        expanding: &mut Vec<SymbolHandle>,
        depth: usize,
    ) -> Option<LinearForm> {
        if expanding.contains(&symbol) {
            return None;
        }
        let initializer = self.local_initializer(symbol, state)?;
        expanding.push(symbol);
        let form = self.linear_form(initializer, state, substitution, expanding, depth + 1);
        expanding.pop();
        form
    }

    fn local_initializer(
        &self,
        symbol: SymbolHandle,
        state: &typed_trees::state::State,
    ) -> Option<ExpressionHandle> {
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let mut initializer = None;
        for statement in statements {
            match statement {
                StatementNode::LocalData(local)
                    if local.symbol == symbol
                        && !local.is_mutable
                        && local.initial_value.is_valid() =>
                {
                    if initializer.is_some() {
                        return None;
                    }
                    initializer = Some(local.initial_value);
                }
                StatementNode::Assignment(assignment) => {
                    let target = self
                        .program
                        .expression_table
                        .peeled_expression_handle(assignment.target);
                    if self
                        .program
                        .expression_table
                        .name_path_node(target)
                        .and_then(|path| path.resolved_symbol())
                        == Some(symbol)
                    {
                        return None;
                    }
                }
                _ => {}
            }
        }
        initializer
    }
}

fn scale_product(left: &LinearForm, right: &LinearForm) -> Option<LinearForm> {
    if let Some(factor) = right.constant_factor() {
        return left.scale(factor);
    }
    if let Some(factor) = left.constant_factor() {
        return right.scale(factor);
    }
    None
}

impl LinearForm {
    fn constant_factor(&self) -> Option<i64> {
        self.terms.is_empty().then_some(self.constant)
    }
}

fn reconstruction_relation(
    operator: BinaryOperator,
    result_on_left: bool,
) -> Option<ReconstructionRelation> {
    match (operator, result_on_left) {
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
            Some(ReconstructionRelation::AtLeast)
        }
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
            Some(ReconstructionRelation::AtMost)
        }
        (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
            Some(ReconstructionRelation::Above)
        }
        (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
            Some(ReconstructionRelation::Below)
        }
        (BinaryOperator::Equal, _) => Some(ReconstructionRelation::Equal),
        _ => None,
    }
}

fn window_entails(
    relation: ReconstructionRelation,
    floor: &LinearForm,
    ceiling: &LinearForm,
    bound: &LinearForm,
    scalars: &ExitScalars<'_, '_>,
    state: &typed_trees::state::State,
) -> bool {
    let holds = |residual: Option<LinearForm>, slack: i64| -> bool {
        residual
            .and_then(|form| form.minimum(scalars, state))
            .is_some_and(|minimum| minimum >= slack)
    };
    match relation {
        ReconstructionRelation::AtLeast => holds(floor.subtract(bound), 0),
        ReconstructionRelation::AtMost => holds(bound.subtract(ceiling), 0),
        ReconstructionRelation::Above => holds(floor.subtract(bound), 1),
        ReconstructionRelation::Below => holds(bound.subtract(ceiling), 1),
        ReconstructionRelation::Equal => floor == ceiling && floor == bound,
    }
}

struct CopiedCall {
    target_symbol: SymbolHandle,
    receiver: ExpressionHandle,
    arguments: arena::HandleSpan<ExpressionHandle>,
    blocked: bool,
}

fn exact_return(
    scalars: &ExitScalars<'_, '_>,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    super::exact_integer_carrier(scalars.program, reference).is_some()
}

/// The one value this state returns: a trailing expression, or one transition
/// whose only target is a value. Locals may precede it. Any other statement
/// is a different control shape and is not inlined.
fn callee_return_expression(
    scalars: &ExitScalars<'_, '_>,
    state: &typed_trees::state::State,
) -> Option<ExpressionHandle> {
    let statements = scalars
        .program
        .statement_table
        .statements(state.statement_nodes);
    let mut returned = None;
    for statement in statements {
        match statement {
            StatementNode::LocalData(_) => {}
            StatementNode::Expression(value) if returned.is_none() => returned = Some(*value),
            StatementNode::Transition(transition) if returned.is_none() => {
                returned = Some(single_transition_value(scalars, transition)?);
            }
            _ => return None,
        }
    }
    returned
}

fn single_transition_value(
    scalars: &ExitScalars<'_, '_>,
    transition: &typed_trees::statement::TableTransition,
) -> Option<ExpressionHandle> {
    let mut value = None;
    for target in transition.targets() {
        match scalars.program.statement_table.transition_target(target) {
            typed_trees::statement::TransitionTargetNode::Value(expression) if value.is_none() => {
                value = Some(*expression);
            }
            _ => return None,
        }
    }
    value
}
