//! Test the sum's case before borrowing any payload. The selected helper
//! reports that case, then visits its payload fields in declaration order.

use super::{
    Bound, CallbackSite, Served, Stop, binding_name, callback, context_type, declaration,
    description_type, expression, field_borrow, field_query_entry, name, named_type,
    satisfies_binder, unserved,
};
use crate::source_state::TypedTrees;
use crate::source_state::data::{
    DataField, DataMember, DataVariant, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
    StaticSymbolApplication, TableBinaryExpression, TableCallExpression, TableDestructurePattern,
    TableStructLiteral, TableStructLiteralField,
};
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::reflection::construction::{
    construct_semantic_schema_graph_for_type, schema_query_authority,
};
use crate::source_state::reflection::{
    CaseDescription, DeclarationDescription, SchemaNode, SchemaShape,
};
use crate::source_state::signature::{StateParameter, StateSignature};
use crate::source_state::state::State;
use crate::source_state::statement::{
    StatementHandle, StatementNode, TableCall, TableTransition, TransitionGuardNode,
    TransitionPattern, TransitionTargetNode,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::ReferenceAccess;
use language_core::core_vocabulary::ReflectionDeclaration;
use numeric_literals::{IntegerLiteral, IntegerRadix};
use numerics::arithmetic::ArithmeticDomain;
use source_text::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};

struct ActiveCase {
    variant: DataVariant,
    description: CaseDescription,
    fields: Vec<DataField>,
}

struct Sum {
    symbol: SymbolHandle,
    name: Identifier,
    description: DeclarationDescription,
    cases: Vec<ActiveCase>,
}

pub(super) fn expand(
    program: &mut TypedTrees,
    holder: &Machine,
    state: &State,
    site: StatementHandle,
    call: &TableCall,
    access: ReferenceAccess,
) -> Served<Vec<StatementNode>> {
    let visitor = declaration(program, call.target_symbol)
        .ok_or(unserved("the sum visitor lost its declaration"))?;
    let query = visitor.symbol;
    let lexical_owner = call
        .authored_machine_application
        .is_valid()
        .then(|| {
            program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .lexical_owner
        })
        .filter(|owner| owner.is_valid())
        .ok_or(unserved("the sum visitor lost its lexical author"))?;
    let parameters = program.machine_callable_parameters(visitor);
    let [subject_binder, case_binder, field_binder, context_binder] = parameters.as_slice() else {
        return Err(Stop::Defect(
            "the sum visitor has the wrong static parameters",
        ));
    };
    let case_binder = structural_binder(case_binder)?;
    let field_binder = structural_binder(field_binder)?;
    let arguments = program
        .expression_table
        .static_machine_application_arguments(call.authored_machine_application)
        .to_vec();
    let (subject, case_visitor, field_visitor, written_context) = match arguments.as_slice() {
        [subject, case_visitor, field_visitor] => (subject, case_visitor, field_visitor, None),
        [subject, case_visitor, field_visitor, context] => {
            (subject, case_visitor, field_visitor, Some(context))
        }
        _ => {
            return Err(unserved(
                "the sum visitor needs the sum type and both callbacks",
            ));
        }
    };
    let [_, context] = program.statement_table.expression_handles(call.arguments) else {
        return Err(unserved("the sum visitor needs a subject and a context"));
    };
    let context = context_type(program, state, *context, written_context)?;
    let type_reference = named_type(program, subject, lexical_owner)?;
    let mut sum = sum_description(program, type_reference, lexical_owner)?;
    if let Some(empty) = typed_trees_to_terminal_psi::lowering::proved_empty_sum_cases(
        program,
        type_reference,
        lexical_owner,
    ) {
        if empty.len() != sum.cases.len() {
            return Err(Stop::Defect(
                "case coverage uses a different sum declaration",
            ));
        }
        let mut positions = empty.into_iter();
        sum.cases
            .retain(|_| !positions.next().expect("one status for each declared case"));
    }
    let application = sum_application(program, subject, type_reference, lexical_owner)?;
    let case_visitor = callback(program, case_visitor)?;
    let field_visitor = callback(program, field_visitor)?;
    let included_types = sum
        .cases
        .iter()
        .flat_map(|case| &case.fields)
        .map(|field| field.type_reference)
        .collect::<Vec<_>>();
    let substitutions = [
        (subject_binder.symbol, Bound::Type(type_reference)),
        (context_binder.symbol, Bound::Type(context)),
    ];
    let operation = format!("reflect::{}", program.symbols.name(query));
    for (binder, visitor, applications) in [
        (case_binder, &case_visitor, None),
        (
            field_binder,
            &field_visitor,
            Some(included_types.as_slice()),
        ),
    ] {
        if program
            .authored_service_reach_rows_for(binder.symbol)
            .next()
            .is_some()
        {
            return Err(unserved(
                "a callback binder needs a refinement proof for its full interface",
            ));
        }
        satisfies_binder(
            program,
            binder,
            &visitor.machine,
            &operation,
            &substitutions,
            applications,
        )?;
    }
    let case_info = description_type(program, case_binder, ReflectionDeclaration::CaseInfo)?;
    let field_info = description_type(program, field_binder, ReflectionDeclaration::FieldInfo)?;
    let field_query = field_query_entry(program, query)?;
    let span = call.source_span;
    let context_reference = reference(program, context, ReferenceAccess::Mutable);
    let subject_reference = reference(program, type_reference, access);
    let dispatcher = GeneratedMachine::new(
        program,
        holder.symbol,
        &format!("visit_cases#{}", site.arena_index()),
        [
            ("value".to_owned(), subject_reference),
            ("context".to_owned(), context_reference),
        ],
    );
    let subject = dispatcher.parameters[0].symbol;
    let context = dispatcher.parameters[1].symbol;
    let mut arms = Vec::with_capacity(sum.cases.len());
    for case in &sum.cases {
        let mut inputs = vec![("context".to_owned(), context_reference)];
        inputs.extend(case.fields.iter().enumerate().map(|(position, field)| {
            (
                format!("field_{position}"),
                reference(program, field.type_reference, access),
            )
        }));
        let branch = GeneratedMachine::new(
            program,
            holder.symbol,
            &format!("visit_case#{}.{}", site.arena_index(), arms.len()),
            inputs,
        );
        let branch_context = branch.parameters[0].symbol;
        let emission = CallbackSite {
            state: branch.entry,
            site,
            origin: query,
            lexical_owner,
            call,
        };
        let mut body = Vec::new();
        let description = case_literal(
            program,
            case_info,
            &sum.description,
            &case.description,
            span,
        )?;
        emission.invoke(
            program,
            &case_visitor,
            branch_context,
            case_info,
            description,
            None,
            &[],
            &mut body,
        );

        let mut actuals = vec![binding_name(program, context, span)];
        for (field, parameter) in case.fields.iter().zip(&branch.parameters[1..]) {
            let declaration = field_declaration(
                program,
                &sum,
                application.as_ref(),
                &case.variant,
                field,
                span,
            );
            let value = binding_name(program, parameter.symbol, span);
            emission.field(
                program,
                &field_query,
                field_info,
                &field_visitor,
                branch_context,
                field,
                declaration,
                Some(value),
                &mut body,
            );
            actuals.push(field_borrow(
                program,
                subject,
                field,
                Some(case.variant.name.clone()),
                access,
                span,
            ));
        }
        let entry = branch.entry;
        let branch_name = branch.name.clone();
        branch.install(program, body);
        let arguments = program.expression_table.insert_expression_handles(actuals);
        let visit = expression(
            program,
            ExpressionNode::Call(TableCallExpression {
                target_symbol: entry,
                target: branch_name,
                arguments,
                operational_acknowledgement: call.operational_acknowledgement,
                ..Default::default()
            }),
            span,
        );
        arms.push(case_arm(program, &sum, &case.variant, subject, visit, span));
    }
    if arms.is_empty() {
        arms.push(empty_dispatch(program, subject, span));
    }
    let entry = dispatcher.entry;
    let target = dispatcher.name.clone();
    dispatcher.install(program, arms);
    // Ordinary call arguments evaluate both operands once, in source order.
    // The application's two callback interfaces have already been checked.
    Ok(vec![StatementNode::Call(TableCall {
        target_symbol: entry,
        target,
        arguments: call.arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: call.discards_result,
        source_span: span,
        ..Default::default()
    })])
}

fn structural_binder(parameter: &TypeParameter) -> Served<&StateSignature> {
    match &parameter.kind {
        TypeParameterKind::Machine {
            contract: MachineParameterContract::Structural(binder),
        } => Ok(binder),
        _ => Err(Stop::Defect(
            "the sum visitor lost its structural callback binder",
        )),
    }
}

fn sum_description(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    owner: SymbolHandle,
) -> Served<Sum> {
    let data = program
        .closed_data_definition(reference)
        .ok_or(unserved("the active-case visitor needs an exact sum type"))?;
    if !program
        .symbols
        .same_symbol_source_package(owner, data.symbol)
    {
        return Err(unserved(
            "a sum declared in another package needs an owning visitation operation",
        ));
    }
    let authority = schema_query_authority(program, owner, data.symbol).map_err(unserved)?;
    let graph = construct_semantic_schema_graph_for_type(program, reference, authority)
        .map_err(unserved)?;
    let description = graph.declaration();
    if description.shape != SchemaShape::Sum {
        return Err(Stop::Rejected(
            "active-case visitation takes a sum. To visit a sum held in a record, borrow its field"
                .to_owned(),
        ));
    }
    let members = program.data_members(data);
    if members.len() != description.members.len() {
        return Err(Stop::Defect(
            "the sum schema does not cover every declared member",
        ));
    }
    let mut cases = Vec::new();
    for (member, description) in members.iter().zip(&description.members) {
        match (member, graph.node(*description)) {
            (DataMember::Variant(variant), SchemaNode::Case(description)) => {
                cases.push(ActiveCase {
                    variant: variant.clone(),
                    description: description.clone(),
                    fields: program
                        .data_payload_fields(variant)
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .cloned()
                        .collect(),
                });
            }
            _ => {
                return Err(Stop::Defect(
                    "sum fields and cases disagree with their schema",
                ));
            }
        }
    }
    Ok(Sum {
        symbol: data.symbol,
        name: data.name.clone(),
        description: description.clone(),
        cases,
    })
}

/// The field query needs the same closed type used by the runtime projection.
fn sum_application(
    program: &mut TypedTrees,
    subject: &StaticMachineArgument,
    reference: TypeReferenceHandle,
    author: SymbolHandle,
) -> Served<Option<StaticMachineArgument>> {
    let mut selected = subject.clone();
    if selected.application.is_none() {
        let TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } = program.type_reference_table.type_reference(reference)
        else {
            return Ok(None);
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .map(|reference| {
                let mut operand = program.static_operand_from_type_reference(*reference)?;
                operand.type_reference = *reference;
                Ok(operand)
            })
            .collect::<Result<Box<[_]>, &'static str>>()
            .map_err(unserved)?;
        selected = StaticMachineArgument {
            symbol: *base_symbol,
            path: Box::new([base_name.clone()]),
            application: Some(Box::new(StaticSymbolApplication {
                arguments,
                lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                member: Default::default(),
            })),
            ..Default::default()
        };
    }
    selected.type_reference = reference;
    program
        .type_reference_table
        .retain_operand_lifetime_owner(reference, author)
        .map_err(unserved)?;
    program
        .static_operand_region_supplier(&selected, author)
        .map_err(unserved)?;
    Ok(Some(selected))
}

fn field_declaration(
    program: &mut TypedTrees,
    sum: &Sum,
    application: Option<&StaticMachineArgument>,
    case: &DataVariant,
    field: &DataField,
    span: SourceSpan,
) -> ExpressionHandle {
    let declaration = name(
        program,
        vec![sum.name.clone(), case.name.clone(), field.name.clone()],
        vec![sum.symbol, case.symbol, field.symbol],
        span,
    );
    if let Some(mut applied) = application.cloned() {
        let field = program
            .expression_table
            .insert_static_member(StaticMachineArgument {
                symbol: field.symbol,
                path: Box::new([field.name.clone()]),
                ..Default::default()
            });
        let member = program
            .expression_table
            .insert_static_member(StaticMachineArgument {
                symbol: case.symbol,
                path: Box::new([case.name.clone()]),
                application: Some(Box::new(StaticSymbolApplication {
                    member: field,
                    arguments: Box::default(),
                    lifetime_arguments: Box::default(),
                })),
                ..Default::default()
            });
        applied
            .application
            .as_mut()
            .expect("retained type application")
            .member = member;
        let operand = program.expression_table.insert_static_member(applied);
        if let ExpressionNode::Name(path) = program.expression_table.expression_mut(declaration) {
            path.static_application = operand;
        }
    }
    declaration
}

fn reference(
    program: &mut TypedTrees,
    referee: TypeReferenceHandle,
    access: ReferenceAccess,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        })
}

struct GeneratedMachine {
    symbol: SymbolHandle,
    entry: SymbolHandle,
    name: Identifier,
    parameters: Vec<StateParameter>,
}

impl GeneratedMachine {
    fn new(
        program: &mut TypedTrees,
        origin: SymbolHandle,
        spelling: &str,
        inputs: impl IntoIterator<Item = (String, TypeReferenceHandle)>,
    ) -> Self {
        let symbol =
            program
                .symbols
                .insert_generated_root_from(origin, SymbolKind::Machine, spelling);
        let entry = program
            .symbols
            .insert_generated_children_from(symbol, [(origin, SymbolKind::State, "entry")])
            .handle_at(0);
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let parameters = program.symbols.insert_generated_children_from(
            entry,
            inputs
                .iter()
                .map(|(name, _)| (origin, SymbolKind::Parameter, name.as_str())),
        );
        let parameters = inputs
            .into_iter()
            .zip(parameters.handles())
            .map(|((name, type_reference), symbol)| StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference,
                ..Default::default()
            })
            .collect();
        Self {
            symbol,
            entry,
            name: Identifier::generated(spelling),
            parameters,
        }
    }

    fn install(self, program: &mut TypedTrees, body: Vec<StatementNode>) {
        let mut statement_nodes = arena::HandleSpan::empty();
        for statement in body {
            program
                .statement_table
                .push_statement(&mut statement_nodes, statement);
        }
        let mut entry = State {
            symbol: self.entry,
            name: Identifier::generated("entry"),
            statement_nodes,
            ..Default::default()
        };
        for parameter in self.parameters {
            program.push_state_parameter(&mut entry, parameter);
        }
        let states = program.machine_states.insert_many([entry]);
        program.push_machine(Machine {
            symbol: self.symbol,
            name: self.name,
            states,
            ..Default::default()
        });
    }
}

fn empty_dispatch(
    program: &mut TypedTrees,
    subject: SymbolHandle,
    span: SourceSpan,
) -> StatementNode {
    let subject = binding_name(program, subject, span);
    let patterns = program
        .statement_table
        .insert_transition_patterns([TransitionPattern {
            is_structural: true,
            subject,
            pattern: TableDestructurePattern::default(),
        }]);
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Terminal);
    StatementNode::Transition(TableTransition {
        patterns,
        target,
        source_span: span,
        ..Default::default()
    })
}

fn case_arm(
    program: &mut TypedTrees,
    sum: &Sum,
    case: &DataVariant,
    subject: SymbolHandle,
    visit: ExpressionHandle,
    span: SourceSpan,
) -> StatementNode {
    let subject = binding_name(program, subject, span);
    let head = name(
        program,
        vec![sum.name.clone(), case.name.clone()],
        vec![sum.symbol, case.symbol],
        span,
    );
    let guard = expression(
        program,
        ExpressionNode::Binary(TableBinaryExpression {
            left: subject,
            right: head,
            operator: BinaryOperator::CaseMembership,
            policy: ArithmeticDomain::Exact,
        }),
        span,
    );
    let truth = expression(program, ExpressionNode::Boolean(true), span);
    let condition = expression(
        program,
        ExpressionNode::Binary(TableBinaryExpression {
            left: guard,
            right: truth,
            operator: BinaryOperator::And,
            policy: ArithmeticDomain::Exact,
        }),
        span,
    );
    let patterns = program
        .statement_table
        .insert_transition_patterns([TransitionPattern {
            is_structural: true,
            subject,
            pattern: TableDestructurePattern {
                head,
                has_rest: true,
                condition,
                ..Default::default()
            },
        }]);
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(visit));
    StatementNode::Transition(TableTransition {
        patterns,
        target,
        guard: TransitionGuardNode::When(guard),
        source_span: span,
        ..Default::default()
    })
}

fn case_literal(
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    sum: &DeclarationDescription,
    case: &CaseDescription,
    span: SourceSpan,
) -> Served<ExpressionHandle> {
    let data = program
        .closed_data_definition(reference)
        .ok_or(Stop::Defect("the case callback lost its description type"))?;
    let type_symbol = data.symbol;
    let type_name = data.name.clone();
    let fields = program
        .data_members(data)
        .iter()
        .map(|member| {
            let DataMember::Field(field) = member else {
                return Err(Stop::Defect("CaseInfo must be a record"));
            };
            Ok((field.name.clone(), field.symbol))
        })
        .collect::<Served<Vec<_>>>()?;
    let owner_application = sum
        .application_identity
        .as_deref()
        .ok_or(Stop::Defect("the selected case has no owner application"))?;
    let bytes =
        |text: &str| ExpressionNode::String(source_text::SourceBytes::from(text.as_bytes()));
    let integer = |value: u64| {
        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, &value.to_string())
            .map(ExpressionNode::Integer)
            .map_err(Stop::Defect)
    };
    let values = [
        ("member_identity", bytes(&case.member_identity)),
        ("owner_identity", bytes(&sum.owner_identity)),
        ("owner_application", bytes(owner_application)),
        ("name", bytes(&case.name)),
        (
            "declaration_position",
            integer(u64::from(case.declaration_position))?,
        ),
        (
            "has_stable_number",
            ExpressionNode::Boolean(case.stable_number.is_some()),
        ),
        ("stable_number", integer(case.stable_number.unwrap_or(0))?),
    ];
    if fields.len() != values.len() {
        return Err(Stop::Defect("CaseInfo has a different field roster"));
    }
    let fields = values
        .into_iter()
        .map(|(key, node)| {
            let (name, field_symbol) = fields
                .iter()
                .find(|(name, _)| name.as_str() == key)
                .ok_or(Stop::Defect("CaseInfo is missing a description field"))?;
            let value = expression(program, node, span);
            Ok(TableStructLiteralField {
                name: name.clone(),
                field_symbol: *field_symbol,
                value,
            })
        })
        .collect::<Served<Vec<_>>>()?;
    let fields = program.expression_table.insert_struct_fields(fields);
    Ok(expression(
        program,
        ExpressionNode::StructLiteral(TableStructLiteral {
            application_type: reference,
            type_name,
            type_symbol,
            fields,
            ..Default::default()
        }),
        span,
    ))
}
