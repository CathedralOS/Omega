//! Invocation-owned observations of exact result producer inputs.
//!
//! A copied result is not progress by itself. Reuse the result this producer
//! already issued for the same occurrence and unchanged input graph, including
//! inputs whose source admission is still pending. These records establish no
//! type equality, binder permission or callable admission. They end with the
//! specialization invocation; there is no cross-program cache or global epoch.
//! Input handles alone are insufficient: const preparation mutates their nodes
//! and integer meaning in place. The existing reference visitor captures only
//! this derivation's reachable inputs, retaining node and custody payloads.

use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{
        ExpressionHandle, ExpressionNode, IndexExpressionAuthority, StaticMachineArgument,
        TableCallExpression, TableMatchArm, TableStructLiteralField,
    },
    name::Identifier,
    references::{self, TypedReferences},
    static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineLifetimeBinding,
    },
    types::{
        ConstIntegerMeaning, TypeConstantOrigin, TypeConstraintNode, TypeReferenceHandle,
        TypeReferenceNode,
    },
};

#[derive(Default)]
pub(crate) struct ResultDerivations {
    rows: Vec<ResultDerivation>,
}

struct ResultDerivation {
    expression: ExpressionHandle,
    generic: bool,
    inputs: ResultInputs,
    result: TypeReferenceHandle,
}

impl ResultDerivations {
    pub(super) fn result(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        generic: bool,
        inputs: &ResultInputs,
    ) -> TypeReferenceHandle {
        self.rows
            .iter()
            .find(|row| {
                row.expression == expression
                    && row.generic == generic
                    && &row.inputs == inputs
                    && program
                        .type_reference_table
                        .contains_type_reference(row.result)
            })
            .map_or_else(TypeReferenceHandle::invalid, |row| row.result)
    }

    pub(super) fn retain(
        &mut self,
        expression: ExpressionHandle,
        generic: bool,
        inputs: ResultInputs,
        result: TypeReferenceHandle,
    ) {
        let row = ResultDerivation {
            expression,
            generic,
            inputs,
            result,
        };
        if let Some(previous) = self
            .rows
            .iter_mut()
            .find(|previous| previous.expression == expression && previous.generic == generic)
        {
            *previous = row;
        } else {
            self.rows.push(row);
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct ResultInputs {
    owner: SymbolHandle,
    declaration: SymbolHandle,
    result: TypeReferenceHandle,
    arguments: Vec<ExpressionHandle>,
    lifetimes: Vec<(Identifier, Option<Identifier>)>,
    bindings: Vec<StaticMachineBinding>,
    parameters: Vec<typed_trees::signature::StateParameter>,
    types: Vec<TypeInput>,
    expressions: Vec<ExpressionInput>,
    applications: Vec<ApplicationInput>,
}

#[derive(PartialEq, Eq)]
struct TypeInput {
    reference: TypeReferenceHandle,
    node: TypeReferenceNode,
    origin: TypeReferenceHandle,
    constants: Vec<TypeConstantOrigin>,
    lifetime_owner: SymbolHandle,
    integer_meaning: ConstIntegerMeaning,
    completed_const_result: Option<language_semantics::const_value::CanonicalConstIdentity>,
    constraints: Vec<TypeConstraintNode>,
}

#[derive(PartialEq, Eq)]
struct ExpressionInput {
    expression: ExpressionHandle,
    node: ExpressionNode,
    original_call: Option<TableCallExpression>,
    result: TypeReferenceHandle,
    authority: Option<IndexExpressionAuthority>,
    source: language_core::expression_origin::ExpressionSourceOrigin,
    selected_match: ExpressionHandle,
    selected_meaning: language_core::expression_origin::ConstMatchIntegerMeaning,
    arms: Vec<TableMatchArm>,
    fields: Vec<TableStructLiteralField>,
    names: Vec<Identifier>,
    member_symbols: Vec<SymbolHandle>,
}

#[derive(PartialEq, Eq)]
struct ApplicationInput {
    application: StaticMachineApplicationHandle,
    registration_operation: SymbolHandle,
    selected_interface: SymbolHandle,
    lexical_owner: SymbolHandle,
    captured_scope: arena::Handle<
        typed_trees::typed_trees::product_applications::CapturedStaticLexicalScopeSlot,
    >,
    target: Identifier,
    receiving_data_parameter: SymbolHandle,
    binding_declaration: SymbolHandle,
    arguments: Vec<StaticMachineArgument>,
    parameters: Vec<SymbolHandle>,
    selected: Vec<StaticMachineArgument>,
    bindings: Vec<StaticMachineBinding>,
    lifetimes: Vec<StaticMachineLifetimeBinding>,
    enclosing: Vec<TypeReferenceHandle>,
}

#[derive(Default)]
struct InputReferences {
    types: Vec<TypeReferenceHandle>,
    expressions: Vec<ExpressionHandle>,
    applications: Vec<StaticMachineApplicationHandle>,
}

impl Extend<SymbolHandle> for InputReferences {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl TypedReferences for InputReferences {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
    fn static_machine_application(&mut self, application: StaticMachineApplicationHandle) -> bool {
        if self.applications.contains(&application) {
            return false;
        }
        self.applications.push(application);
        true
    }
    fn applied_callable_signatures(&mut self, _: StaticMachineApplicationHandle) -> bool {
        // Copied checking views are outputs, not result producer inputs.
        // Any actually selected return/formal graph is observed explicitly.
        false
    }
}

pub(super) fn inputs(
    program: &TypedTrees,
    owner: SymbolHandle,
    declaration: SymbolHandle,
    result: TypeReferenceHandle,
    arguments: &[ExpressionHandle],
    lifetimes: &[(Identifier, Option<Identifier>)],
    application: StaticMachineApplicationHandle,
    bindings: &[StaticMachineBinding],
    parameters: &[typed_trees::signature::StateParameter],
    additional_types: &[TypeReferenceHandle],
) -> Result<ResultInputs, diagnostics::Diagnostic> {
    let mut retained = InputReferences::default();
    references::visit_type_reference(program, &mut retained, result);
    for reference in additional_types {
        references::visit_type_reference(program, &mut retained, *reference);
    }
    for argument in arguments {
        references::visit_expression(program, &mut retained, *argument);
    }
    // Explicit proposals are not yet guaranteed to equal either source or
    // executable application roster. Observe their actual payloads directly.
    for binding in bindings {
        references::visit_static_machine_binding(program, &mut retained, binding);
    }
    for parameter in parameters {
        references::visit_type_reference(program, &mut retained, parameter.type_reference);
    }
    if application.is_valid() {
        references::visit_static_machine_bindings(program, &mut retained, application);
    }
    // The normalization carrier is copied alongside each index expression.
    // The generic reference visitor already visits its selected operation.
    let mut next_expression = 0;
    let mut next_application = 0;
    while next_expression < retained.expressions.len()
        || next_application < retained.applications.len()
    {
        if next_expression < retained.expressions.len() {
            let expression = retained.expressions[next_expression];
            if let Some(authority) = program
                .expression_table
                .index_expression_authority(expression)
                && let Some(normalization) = &authority.normalization
            {
                references::visit_type_reference(program, &mut retained, normalization.index_type);
            }
            next_expression += 1;
        }
        if next_application < retained.applications.len() {
            let application = retained.applications[next_application];
            // The shared binding visitor observes nested provider applications.
            // Their authored operands are copied too, independently of their
            // selected/inferred roster, and may contain mutable type graphs.
            for argument in program
                .expression_table
                .static_machine_application_arguments(application)
            {
                references::visit_static_argument(program, &mut retained, argument);
            }
            next_application += 1;
        }
    }
    let types = retained
        .types
        .into_iter()
        .map(|reference| {
            let table = &program.type_reference_table;
            Ok(TypeInput {
                reference,
                node: table.type_reference(reference).clone(),
                origin: table.data_application_origin(reference),
                constants: table
                    .constant_origins(reference)
                    .map_err(diagnostics::Diagnostic::error)?
                    .to_vec(),
                lifetime_owner: table.operand_lifetime_owner(reference),
                integer_meaning: table.const_integer_meaning(reference),
                completed_const_result: table
                    .completed_const_result(reference)
                    .map(|value| value.identity()),
                constraints: match table.type_reference(reference) {
                    TypeReferenceNode::Constrained { constraints, .. } => {
                        table.constraints(*constraints).to_vec()
                    }
                    _ => Vec::new(),
                },
            })
        })
        .collect::<Result<Vec<_>, diagnostics::Diagnostic>>()?;
    let expressions = retained
        .expressions
        .into_iter()
        .map(|expression| {
            let table = &program.expression_table;
            let node = table.expression(expression);
            let (names, member_symbols) = if let ExpressionNode::Name(path) = node {
                (
                    table.name_path_members(path.members).to_vec(),
                    table.name_path_member_symbols(path.member_symbols).to_vec(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            ExpressionInput {
                expression,
                node: node.clone(),
                original_call: table
                    .call_node(expression)
                    .and_then(|call| table.source_call(call))
                    .cloned(),
                result: table.constructed_value_type_reference(expression),
                authority: table.index_expression_authority(expression).cloned(),
                source: table.expression_source_origin(expression),
                selected_match: table.selected_const_match_result(expression),
                selected_meaning: table.selected_const_match_meaning(expression),
                arms: if let ExpressionNode::Match(dispatch) = node {
                    table.match_arms(dispatch.arms).to_vec()
                } else {
                    Vec::new()
                },
                fields: if let ExpressionNode::StructLiteral(literal) = node {
                    table.struct_fields(literal.fields).to_vec()
                } else {
                    Vec::new()
                },
                names,
                member_symbols,
            }
        })
        .collect();
    let applications = retained
        .applications
        .into_iter()
        .map(|application| {
            let table = &program.expression_table;
            let node = table.static_machine_application(application);
            ApplicationInput {
                application,
                registration_operation: node.registration_operation,
                selected_interface: node.selected_interface,
                lexical_owner: node.lexical_owner,
                captured_scope: node.captured_lexical_scope,
                target: node.target.clone(),
                receiving_data_parameter: node.receiving_data_parameter,
                binding_declaration: node.binding_declaration,
                arguments: table
                    .static_machine_application_arguments(application)
                    .to_vec(),
                parameters: table
                    .static_machine_application_argument_parameters(application)
                    .to_vec(),
                selected: table
                    .static_machine_application_selected_machines(application)
                    .to_vec(),
                bindings: table
                    .static_machine_application_bindings(application)
                    .to_vec(),
                lifetimes: table
                    .static_machine_application_lifetimes(application)
                    .to_vec(),
                enclosing: program
                    .type_reference_table
                    .type_reference_handles(node.enclosing_data_applications)
                    .to_vec(),
            }
        })
        .collect();
    Ok(ResultInputs {
        owner,
        declaration,
        result,
        arguments: arguments.to_vec(),
        lifetimes: lifetimes.to_vec(),
        bindings: bindings.to_vec(),
        parameters: parameters.to_vec(),
        types,
        expressions,
        applications,
    })
}
