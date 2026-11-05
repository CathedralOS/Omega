//! Lower source execution paths without assuming any callable conclusion.
//!
//! Guards retain their source identity and both outcomes. Only a canonical
//! case test supplies a constructor equation; no theorem search removes a path.
//! The later graph proof may overapproximate a guard, but must cover each real
//! source return and preserve every call's selected application and actuals.

use super::{OrdinalCarrier, SourceOrderValue};
use crate::machine_calls::calls::{CallSite, call_site_argument_expressions};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementHandle, StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use typed_trees::types::TypeReferenceHandle;

/// Exact occurrences in the immutable typed input retain the complete authored
/// applications, including static operands and evidence. No copied call record
/// substitutes for the original occurrence during argument authentication.
#[derive(Clone, Copy, Debug)]
pub enum SourceApplication {
    Expression { expression: ExpressionHandle },
    Statement { statement: StatementHandle },
    Transition { target: TransitionTargetHandle },
}

impl SourceApplication {
    fn site<'program>(
        &self,
        program: &'program dyn crate::CheckingInput,
    ) -> Result<CallSite<'program>, &'static str> {
        match *self {
            Self::Expression { expression } => {
                let call = program
                    .expression_table
                    .call_node(expression)
                    .ok_or("the source comparison expression lost its exact call occurrence")?;
                Ok(CallSite::Expression { expression, call })
            }
            Self::Statement { statement } => {
                if !program.statement_table.statement_is_valid(statement) {
                    return Err("the source comparison statement call is unavailable");
                }
                let call = program
                    .statement_table
                    .statement(statement)
                    .as_call()
                    .ok_or("the source comparison statement is not its recorded call")?;
                Ok(CallSite::Statement(call))
            }
            Self::Transition { target } => {
                if !program.statement_table.transition_target_is_valid(target) {
                    return Err("the source comparison transition call is unavailable");
                }
                let TransitionTargetNode::Named {
                    path,
                    static_machine_parameter,
                    arguments,
                    evidence_arguments,
                    source_span,
                    authored_call_selection,
                    ..
                } = program.statement_table.transition_target(target)
                else {
                    return Err("the source call has no named transition application");
                };
                Ok(CallSite::TransitionNamed {
                    path,
                    static_machine_parameter: *static_machine_parameter,
                    arguments: *arguments,
                    evidence_arguments,
                    source_span: *source_span,
                    authored_call_selection: *authored_call_selection,
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceCall {
    pub owner: SymbolHandle,
    pub state: SymbolHandle,
    pub statement: usize,
    pub target: SymbolHandle,
    pub application: SourceApplication,
    pub arguments: Vec<SourceOrderValue>,
    pub result_type: TypeReferenceHandle,
    pub result: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct SourceReturn {
    pub state: SymbolHandle,
    pub statement: usize,
    pub target: Option<TransitionTargetHandle>,
}

#[derive(Clone, Debug)]
pub enum SourceOutcome {
    Value(SourceOrderValue),
    Unit,
    NoNormal,
}

/// The completed source value represented by one logical rank binder.
/// A generated variable is never an unspecified or invented source value.
#[derive(Clone, Debug)]
pub enum SourceValueOrigin {
    Parameter {
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
    },
    CaseChild {
        subject: SourceOrderValue,
        field: SymbolHandle,
        guard: ExpressionHandle,
        selected: bool,
    },
    CallResult {
        call: usize,
    },
}

#[derive(Clone, Debug)]
pub struct SourceCase {
    pub source_return: SourceReturn,
    pub values: Vec<SourceValueOrigin>,
    pub inputs: Vec<SourceOrderValue>,
    pub equations: Vec<(SourceOrderValue, SourceOrderValue)>,
    pub guards: Vec<(ExpressionHandle, bool)>,
    pub calls: Vec<SourceCall>,
    pub outcome: SourceOutcome,
}

#[derive(Clone)]
struct Path {
    values: Vec<SourceValueOrigin>,
    inputs: Vec<SourceOrderValue>,
    bindings: Vec<(SymbolHandle, SourceOrderValue)>,
    equations: Vec<(SourceOrderValue, SourceOrderValue)>,
    guards: Vec<(ExpressionHandle, bool)>,
    calls: Vec<SourceCall>,
    children: Vec<(SourceOrderValue, SourceOrderValue)>,
}

impl Path {
    fn fresh(&mut self, origin: SourceValueOrigin) -> Result<SourceOrderValue, &'static str> {
        let position = self.values.len();
        self.values
            .try_reserve(1)
            .map_err(|_| "the source comparison graph cannot retain another value binder")?;
        self.values.push(origin);
        Ok(SourceOrderValue::Variable(position))
    }

    fn bind(&mut self, symbol: SymbolHandle, value: SourceOrderValue) -> Result<(), &'static str> {
        if !symbol.is_valid() {
            return Err("the source comparison value has no resolved binding");
        }
        if let Some((_, previous)) = self.bindings.iter_mut().find(|(key, _)| *key == symbol) {
            *previous = value;
        } else {
            self.bindings.push((symbol, value));
        }
        Ok(())
    }

    fn finish(self, source_return: SourceReturn, outcome: SourceOutcome) -> SourceCase {
        SourceCase {
            source_return,
            values: self.values,
            inputs: self.inputs,
            equations: self.equations,
            guards: self.guards,
            calls: self.calls,
            outcome,
        }
    }
}

pub(super) fn body(
    program: &dyn crate::CheckingInput,
    carrier: &OrdinalCarrier,
    machine: &Machine,
) -> Result<Vec<SourceCase>, &'static str> {
    let entry = program
        .machine_states(machine)
        .first()
        .ok_or("the source comparison graph has no entry state")?;
    let inputs = program
        .state_parameters(entry)
        .iter()
        .enumerate()
        .map(|(position, _)| SourceOrderValue::Variable(position))
        .collect::<Vec<_>>();
    let bindings = program
        .state_parameters(entry)
        .iter()
        .zip(&inputs)
        .map(|(parameter, value)| (parameter.symbol, value.clone()))
        .collect();
    let path = Path {
        values: program
            .state_parameters(entry)
            .iter()
            .map(|parameter| SourceValueOrigin::Parameter {
                symbol: parameter.symbol,
                reference: parameter.type_reference,
            })
            .collect(),
        inputs,
        bindings,
        equations: Vec::new(),
        guards: Vec::new(),
        calls: Vec::new(),
        children: Vec::new(),
    };
    Reader {
        program,
        carrier,
        machine,
    }
    .state(entry, path, &mut Vec::new())
}

struct Reader<'a> {
    program: &'a dyn crate::CheckingInput,
    carrier: &'a OrdinalCarrier,
    machine: &'a Machine,
}

impl Reader<'_> {
    fn state(
        &self,
        state: &State,
        path: Path,
        active: &mut Vec<SymbolHandle>,
    ) -> Result<Vec<SourceCase>, &'static str> {
        if active.contains(&state.symbol) {
            return Err(
                "the source comparison graph needs a checked recurrent state interpretation",
            );
        }
        active.push(state.symbol);
        let result = self.state_body(state, path, active);
        active.pop();
        result
    }

    fn state_body(
        &self,
        state: &State,
        path: Path,
        active: &mut Vec<SymbolHandle>,
    ) -> Result<Vec<SourceCase>, &'static str> {
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let order = self
            .program
            .statement_table
            .finite_execution_order(state.statement_nodes)
            .map_err(|_| "the source comparison graph has an invalid statement order")?
            .ok_or("the source comparison graph has no finite statement order")?;
        let mut remaining = vec![path];
        let mut returns = Vec::new();
        for (execution, &ordinal) in order.iter().enumerate() {
            let statement = statements
                .get(ordinal)
                .ok_or("the source comparison statement is outside its retained body")?;
            if super::super::is_arm_pattern_marker(statement) {
                continue;
            }
            let mut following = Vec::new();
            for mut path in remaining {
                match statement {
                    StatementNode::LocalData(local) => {
                        let initializer = local
                            .ordinary_value_initializer()
                            .ok_or("the source comparison local has no formed initializer")?;
                        let value = self.value(state, ordinal, initializer, &mut path)?;
                        path.bind(local.symbol, value)?;
                        following.push(path);
                    }
                    StatementNode::Assignment(assignment) => {
                        let value = self.value(state, ordinal, assignment.value, &mut path)?;
                        let ExpressionNode::Name(destination) =
                            self.program.expression_table.expression(assignment.target)
                        else {
                            return Err(
                                "the source comparison graph cannot replace an unresolved subplace",
                            );
                        };
                        path.bind(destination.symbol, value)?;
                        following.push(path);
                    }
                    StatementNode::Call(_) => {
                        self.call(
                            state,
                            ordinal,
                            SourceApplication::Statement { statement: state.statement_nodes.handle_at(u32::try_from(ordinal).map_err(|_| "the source comparison statement ordinal exceeds its retained handle span")?) },
                            &mut path,
                        )?;
                        following.push(path);
                    }
                    StatementNode::Expression(expression) => {
                        let value = self.value(state, ordinal, *expression, &mut path)?;
                        if execution + 1 == order.len() {
                            returns.push(path.finish(
                                SourceReturn {
                                    state: state.symbol,
                                    statement: ordinal,
                                    target: None,
                                },
                                SourceOutcome::Value(value),
                            ));
                        } else {
                            following.push(path);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if transition.continuation.is_valid() {
                            return Err(
                                "the source comparison graph has an unrepresented invocation continuation",
                            );
                        }
                        for (selected, branch) in
                            self.guard(state, ordinal, transition.guard, path)?
                        {
                            if !selected {
                                following.push(branch);
                            } else if transition.exit != TransitionExit::Ordinary {
                                returns.push(branch.finish(
                                    SourceReturn {
                                        state: state.symbol,
                                        statement: ordinal,
                                        target: Some(transition.target),
                                    },
                                    SourceOutcome::NoNormal,
                                ));
                            } else {
                                returns.extend(self.target(
                                    state,
                                    ordinal,
                                    transition.target,
                                    branch,
                                    active,
                                )?);
                            }
                        }
                    }
                    // These are proof statements, not executable premises of
                    // the body graph. Their dependencies stay in the separate
                    // complete structural-call closure.
                    StatementNode::AssemblyFact(_) => following.push(path),
                    StatementNode::Assembly(_) => {
                        return Err("the source comparison graph has no assembly denotation");
                    }
                }
            }
            remaining = following;
        }
        for path in remaining {
            returns.push(path.finish(
                SourceReturn {
                    state: state.symbol,
                    statement: statements.len(),
                    target: None,
                },
                SourceOutcome::Unit,
            ));
        }
        Ok(returns)
    }

    fn guard(
        &self,
        state: &State,
        statement: usize,
        guard: TransitionGuardNode,
        path: Path,
    ) -> Result<Vec<(bool, Path)>, &'static str> {
        match guard {
            TransitionGuardNode::Always => Ok(vec![(true, path)]),
            TransitionGuardNode::When(expression) => {
                self.condition(state, statement, expression, path)
            }
        }
    }

    fn condition(
        &self,
        state: &State,
        statement: usize,
        expression: ExpressionHandle,
        mut path: Path,
    ) -> Result<Vec<(bool, Path)>, &'static str> {
        use typed_trees::expression::BinaryOperator;
        if let ExpressionNode::Boolean(value) = self.program.expression_table.expression(expression)
        {
            path.guards.push((expression, *value));
            return Ok(vec![(*value, path)]);
        }
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            && crate::has_builtin_binary_expression_meaning(
                self.program,
                self.machine,
                Some(state),
                expression,
            )
        {
            let mut outcomes = Vec::new();
            for (left, path) in self.condition(state, statement, binary.left, path)? {
                if (binary.operator == BinaryOperator::And && !left)
                    || (binary.operator == BinaryOperator::Or && left)
                {
                    let mut path = path;
                    path.guards.push((expression, left));
                    outcomes.push((left, path));
                } else {
                    for (right, mut path) in self.condition(state, statement, binary.right, path)? {
                        path.guards.push((expression, right));
                        outcomes.push((right, path));
                    }
                }
            }
            return Ok(outcomes);
        }
        let (definition, variant) = super::super::structural_terms::case_guard_classifier(
            self.program,
            self.machine,
            Some(state),
            expression,
        )
        .ok_or("the source comparison guard has no complete Boolean execution graph")?;
        let selected = self
            .program
            .data_constructor_definition(self.carrier.reference, definition.symbol)
            .ok_or("the source comparison guard has no exact carrier application")?;
        let case = self
            .program
            .data_constructor_case(self.carrier.reference, definition.symbol, variant.symbol)
            .ok_or("the source comparison guard has no exact case application")?;
        if selected.symbol != self.carrier.declaration {
            return Err("the source comparison guard selects another carrier");
        }
        let comparison = self
            .program
            .expression_table
            .binary_node(expression)
            .ok_or("the source comparison case guard has no retained subject")?;
        // Complete the subject once, then fork its actual result and calls.
        let subject = self.value(state, statement, comparison.left, &mut path)?;
        // A previous test of this same completed value already chose its tag.
        // This is ordinary case execution, not a proof conclusion or an
        // assumption from the comparison's contract.
        let known = match &subject {
            SourceOrderValue::Base => Some(self.carrier.base),
            SourceOrderValue::Successor(_) => Some(self.carrier.successor),
            SourceOrderValue::Variable(_) => {
                path.equations
                    .iter()
                    .rev()
                    .find_map(|(value, constructor)| {
                        if value != &subject {
                            return None;
                        }
                        match constructor {
                            SourceOrderValue::Base => Some(self.carrier.base),
                            SourceOrderValue::Successor(_) => Some(self.carrier.successor),
                            SourceOrderValue::Variable(_) => None,
                        }
                    })
            }
        };
        if let Some(known) = known {
            let selected = known == case.symbol;
            path.guards.push((expression, selected));
            return Ok(vec![(selected, path)]);
        }
        let mut other = path.clone();
        path.guards.push((expression, true));
        other.guards.push((expression, false));
        self.case_equation(&mut path, subject.clone(), case.symbol, expression, true)?;
        let opposite = if case.symbol == self.carrier.base {
            self.carrier.successor
        } else if case.symbol == self.carrier.successor {
            self.carrier.base
        } else {
            return Err("the source comparison guard is outside its unary case roster");
        };
        self.case_equation(&mut other, subject, opposite, expression, false)?;
        Ok(vec![(true, path), (false, other)])
    }

    fn case_equation(
        &self,
        path: &mut Path,
        subject: SourceOrderValue,
        case: SymbolHandle,
        guard: ExpressionHandle,
        selected: bool,
    ) -> Result<(), &'static str> {
        let value = if case == self.carrier.base {
            SourceOrderValue::Base
        } else if case == self.carrier.successor {
            let child = path.fresh(SourceValueOrigin::CaseChild {
                subject: subject.clone(),
                field: self.carrier.child,
                guard,
                selected,
            })?;
            path.children.push((subject.clone(), child.clone()));
            SourceOrderValue::Successor(Box::new(child))
        } else {
            return Err("the source comparison pattern lost its declared case");
        };
        path.equations.push((subject, value));
        Ok(())
    }

    fn target(
        &self,
        state: &State,
        statement: usize,
        target: TransitionTargetHandle,
        mut path: Path,
        active: &mut Vec<SymbolHandle>,
    ) -> Result<Vec<SourceCase>, &'static str> {
        if !self
            .program
            .statement_table
            .transition_target_is_valid(target)
        {
            return Err("the source comparison transition has no exact destination");
        }
        let node = self.program.statement_table.transition_target(target);
        let source_return = SourceReturn {
            state: state.symbol,
            statement,
            target: Some(target),
        };
        match node {
            TransitionTargetNode::Value(expression) => {
                let value = self.value(state, statement, *expression, &mut path)?;
                Ok(vec![
                    path.finish(source_return, SourceOutcome::Value(value)),
                ])
            }
            TransitionTargetNode::Terminal => {
                Ok(vec![path.finish(source_return, SourceOutcome::Unit)])
            }
            TransitionTargetNode::Named {
                path: destination, ..
            } if destination.symbol == self.carrier.base => {
                let site = SourceApplication::Transition { target }.site(self.program)?;
                if !call_site_argument_expressions(self.program, &site).is_empty() {
                    return Err(
                        "the source comparison base constructor has unexpected executable operands",
                    );
                }
                Ok(vec![path.finish(
                    source_return,
                    SourceOutcome::Value(SourceOrderValue::Base),
                )])
            }
            TransitionTargetNode::Named { .. } => {
                let application = SourceApplication::Transition { target };
                let site = application.site(self.program)?;
                if site.is_internal_state_transfer(self.program, self.machine.symbol) {
                    let selected = self
                        .program
                        .state_in_machine(self.machine, site.target_symbol(self.program))
                        .ok_or("the source comparison state transfer has no exact arrival")?;
                    let arguments = self.arguments(state, statement, &site, &mut path)?;
                    let parameters = self.program.state_parameters(selected);
                    if parameters.len() != arguments.len() {
                        return Err("the source comparison state arrival has incomplete actuals");
                    }
                    for (parameter, argument) in parameters.iter().zip(arguments) {
                        path.bind(parameter.symbol, argument)?;
                    }
                    self.state(selected, path, active)
                } else {
                    let value = self.call(state, statement, application, &mut path)?;
                    Ok(vec![path.finish(
                        source_return,
                        value.map_or(SourceOutcome::Unit, SourceOutcome::Value),
                    )])
                }
            }
        }
    }

    fn arguments(
        &self,
        state: &State,
        statement: usize,
        site: &CallSite<'_>,
        path: &mut Path,
    ) -> Result<Vec<SourceOrderValue>, &'static str> {
        let expressions = call_site_argument_expressions(self.program, site);
        let mut values = vec![None; expressions.len()];
        for position in site.argument_evaluation_order(self.program)? {
            let expression = *expressions
                .get(position)
                .ok_or("the source comparison call has an invalid operand order")?;
            let value = self.value(state, statement, expression, path)?;
            let destination = values
                .get_mut(position)
                .ok_or("the source comparison call operand is outside its roster")?;
            if destination.replace(value).is_some() {
                return Err("the source comparison call evaluates an operand twice");
            }
        }
        values
            .into_iter()
            .map(|value| value.ok_or("the source comparison call omits an operand"))
            .collect()
    }

    fn call(
        &self,
        state: &State,
        statement: usize,
        application: SourceApplication,
        path: &mut Path,
    ) -> Result<Option<SourceOrderValue>, &'static str> {
        let site = application.site(self.program)?;
        let target = site.target_symbol(self.program);
        let arguments = self.arguments(state, statement, &site, path)?;
        let selected =
            super::super::structural_terms::selected_application_machine(self.program, target)
                .ok_or("the source comparison call has no exact selected entry body")?;
        let entry = self
            .program
            .machine_states(selected)
            .first()
            .ok_or("the source comparison call has no retained entry")?;
        if self.program.state_parameters(entry).len() != arguments.len() {
            return Err("the source comparison call lost its complete actual/formal roster");
        }
        let result_type = crate::call_target_return_type(self.program, target)
            .ok_or("the source comparison call has no exact result carrier")?;
        let result = if self.program.package_qualified_type_identity(result_type)
            == self.carrier.identity
        {
            let SourceOrderValue::Variable(position) =
                path.fresh(SourceValueOrigin::CallResult {
                    call: path.calls.len(),
                })?
            else {
                unreachable!()
            };
            Some(position)
        } else if super::super::structural_terms::selected_application_machine(self.program, target)
            .is_some_and(|machine| crate::machine_has_unit_result(self.program, machine))
        {
            None
        } else {
            return Err("the source comparison call returns an unrepresented value carrier");
        };
        path.calls.push(SourceCall {
            owner: self.machine.symbol,
            state: state.symbol,
            statement,
            target,
            application,
            arguments,
            result_type,
            result,
        });
        Ok(result.map(SourceOrderValue::Variable))
    }

    fn value(
        &self,
        state: &State,
        statement: usize,
        expression: ExpressionHandle,
        path: &mut Path,
    ) -> Result<SourceOrderValue, &'static str> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return Err("the source comparison value has no retained expression");
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                if let Some((_, value)) = path
                    .bindings
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == name.symbol)
                {
                    return Ok(value.clone());
                }
                if let Some((definition, variant)) =
                    super::super::structural_terms::case_classifier(self.program, expression)
                {
                    let selected = self
                        .program
                        .data_constructor_definition(self.carrier.reference, definition.symbol)
                        .ok_or("the source comparison literal has no exact carrier")?;
                    let case = self
                        .program
                        .data_constructor_case(
                            self.carrier.reference,
                            definition.symbol,
                            variant.symbol,
                        )
                        .ok_or("the source comparison literal has no exact case")?;
                    if selected.symbol == self.carrier.declaration
                        && case.symbol == self.carrier.base
                    {
                        return Ok(SourceOrderValue::Base);
                    }
                }
                Err("the source comparison name has no exact supplying value")
            }
            ExpressionNode::StructLiteral(literal) => {
                let definition = self
                    .program
                    .data_constructor_definition(self.carrier.reference, literal.type_symbol)
                    .ok_or("the source comparison constructor has no exact carrier")?;
                let variant = literal
                    .case_symbol
                    .and_then(|symbol| {
                        self.program.data_constructor_case(
                            self.carrier.reference,
                            literal.type_symbol,
                            symbol,
                        )
                    })
                    .ok_or("the source comparison constructor has no exact case")?;
                if definition.symbol != self.carrier.declaration {
                    return Err("the source comparison constructor belongs to another carrier");
                }
                let fields = self.program.expression_table.struct_fields(literal.fields);
                if variant.symbol == self.carrier.base && fields.is_empty() {
                    return Ok(SourceOrderValue::Base);
                }
                let [field] = fields else {
                    return Err("the source comparison constructor lost its complete field roster");
                };
                if variant.symbol != self.carrier.successor
                    || field.field_symbol != self.carrier.child
                {
                    return Err(
                        "the source comparison constructor lost its recursive field identity",
                    );
                }
                Ok(SourceOrderValue::Successor(Box::new(self.value(
                    state,
                    statement,
                    field.value,
                    path,
                )?)))
            }
            ExpressionNode::Member(member) if member.member_symbol == self.carrier.child => {
                let subject = self.value(state, statement, member.receiver, path)?;
                if let SourceOrderValue::Successor(child) = &subject {
                    return Ok(child.as_ref().clone());
                }
                path.children
                    .iter()
                    .rev()
                    .find(|(parent, _)| *parent == subject)
                    .map(|(_, child)| child.clone())
                    .ok_or("the source comparison field has no selected constructor")
            }
            ExpressionNode::Call(_) => self
                .call(
                    state,
                    statement,
                    SourceApplication::Expression { expression },
                    path,
                )?
                .ok_or("the source comparison uses a Unit call as a value"),
            // No unresolved value becomes a fresh rank variable. A binder,
            // source constructor or actual completed call must supply it.
            _ => Err("the source comparison operand has no complete value interpretation"),
        }
    }
}
