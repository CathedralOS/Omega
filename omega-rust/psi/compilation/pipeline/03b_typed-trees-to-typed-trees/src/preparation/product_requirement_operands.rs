//! Prepare a source-issued Product requirement operand after Product joining.
//! This consumes its owned graph, resolves names in the original source scope,
//! and uses the ordinary application owners before immutable checking. It does
//! not execute a source call, choose a provider, or manufacture a new telescope.
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ProductDeclarationOperand, ProductOperandRole, product_operands::*,
};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees, expression as expr,
    name::Identifier,
    trait_definition::{AppliedTraitScope, RetainedTraitScope},
    types::{self, TypeReferenceHandle, TypeReferenceNode},
};

mod expressions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedProductRequirement {
    pub requirement: SymbolHandle,
    pub closed_requirement: Option<SymbolHandle>,
    /// None is the declaration's full universal schema. Some retains the exact
    /// applied outer telescope; even a phantom type remains nominal identity.
    pub trait_scope: Option<RetainedTraitScope>,
}

pub fn prepare_product_requirement_operand(
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    author: SymbolHandle,
) -> Result<PreparedProductRequirement, Vec<Diagnostic>> {
    let refuse =
        |reason: &str| vec![Diagnostic::error(reason).with_source_span(operand.source_span)];
    let Some(graph) = operand.application.as_deref() else {
        return typed_trees::product_operand::resolve_product_path(
            program,
            &operand.path,
            operand.source_span,
            ProductOperandRole::Requirement,
        )
        .map(|requirement| PreparedProductRequirement {
            requirement,
            closed_requirement: None,
            trait_scope: None,
        })
        .ok_or_else(|| {
            refuse("native binding must name one exact visible package-owned requirement")
        });
    };
    let mut lower = Lower {
        program,
        graph,
        author,
        types: Vec::new(),
        expressions: Vec::new(),
        subjects: Vec::new(),
        active_types: Vec::new(),
        active_expressions: Vec::new(),
        active_operands: Vec::new(),
    };
    if !graph.operands.is_valid(graph.root) {
        return Err(refuse("designated requirement has no owned root operand"));
    }
    let root = lower.graph.operands.get(graph.root).clone();
    if !graph.applications.is_valid(root.application) {
        return Err(refuse("designated requirement has no owned application"));
    }
    let root_application = graph.applications.get(root.application).clone();
    let (requirement, environment, method) = if root_application.member.is_valid() {
        let path = lower.path(root.path)?;
        let owner = typed_trees::product_operand::resolve_product_path(
            lower.program,
            &path,
            operand.source_span,
            ProductOperandRole::BoundarySubject,
        )
        .ok_or_else(|| {
            refuse("applied native requirement owner is not one visible boundary declaration")
        })?;
        let application = lower.application_arguments(root.application)?;
        let mut owner_argument = expr::StaticMachineArgument {
            path: Box::from([Identifier::generated(path)]),
            symbol: owner,
            application: Some(Box::new(application)),
            ..Default::default()
        };
        // The member continuation is syntax, not an argument of the owner.
        owner_argument
            .application
            .as_mut()
            .expect("created above")
            .member = Handle::invalid();
        let reference = lower
            .program
            .static_operand_type_reference_in_scope(&owner_argument, author)
            .map_err(refuse)?;
        lower.program.retain_designated_static_type_argument(
            author,
            reference,
            operand.source_span,
        );
        // Closure consumes a derived tuple. Keep the source reference above
        // intact so immutable checking still judges every supplying recipe.
        crate::monomorphization::normalize_const_arguments(
            lower.program,
            author,
            std::slice::from_mut(&mut owner_argument),
        )
        .map_err(|reason| refuse(&reason))?;
        let reference = lower
            .program
            .static_operand_type_reference_in_scope(&owner_argument, author)
            .map_err(refuse)?;
        lower.program.retain_designated_static_type_argument(
            author,
            reference,
            operand.source_span,
        );
        let scope = AppliedTraitScope::from_type_reference(lower.program, reference, author)
            .ok_or_else(|| {
                refuse("native requirement has an invalid or incomplete outer application")
            })?;
        lower
            .program
            .require_receiving_trait_operands(&scope)
            .map_err(refuse)?;
        let environment = scope.environment();
        let member = lower.graph.operands.get(root_application.member).clone();
        let member_path = lower.path(member.path)?;
        let joined = format!(
            "{}::{member_path}",
            lower.program.symbols.display_path(owner, "::")
        );
        let requirement = typed_trees::product_operand::resolve_product_path(
            lower.program,
            &joined,
            operand.source_span,
            ProductOperandRole::Requirement,
        )
        .ok_or_else(|| {
            refuse("applied native requirement has no exact visible member in its selected owner")
        })?;
        if lower
            .program
            .trait_machine_signature(requirement)
            .is_none_or(|(definition, _)| definition.symbol != owner)
        {
            return Err(refuse(
                "applied requirement member does not belong to its exact selected owner",
            ));
        }
        (requirement, Some(environment), member.application)
    } else {
        let requirement = typed_trees::product_operand::resolve_product_path(
            lower.program,
            &operand.path,
            operand.source_span,
            ProductOperandRole::Requirement,
        )
        .ok_or_else(|| {
            refuse("native binding must name one exact visible package-owned requirement")
        })?;
        (requirement, None, root.application)
    };
    // This newly joined source occurrence may introduce a previously unused
    // outer telescope. Prepare its logical signatures through the same owner
    // as ordinary applied requirement calls before requesting an instance.
    lower
        .program
        .prepare_applied_callable_signatures()
        .map_err(|reason| refuse(&reason))?;
    let Some((owner, signature)) = lower.program.trait_machine_signature(requirement) else {
        return Err(refuse(
            "an applied native declaration must be a public requirement member",
        ));
    };
    let environment =
        environment.unwrap_or_else(|| AppliedTraitScope::abstract_scope(owner).environment());
    let parameters = lower
        .program
        .state_signature_type_parameters(signature)
        .to_vec();
    let lifetime_parameters = signature.lifetime_parameters.to_vec();
    let name = signature.name.clone();
    let trait_arguments = lower
        .program
        .materialize_trait_application_arguments(&environment)
        .map_err(refuse)?;

    let supplied = if method.is_valid() {
        Some(lower.application(method)?)
    } else {
        None
    };
    let has_outer = root_application.member.is_valid();
    // An absent method tuple keeps the method's own universal telescope. An
    // explicitly supplied tuple must close completely, including unused args.
    if let Some(supplied) = &supplied {
        if supplied.member.is_valid()
            || supplied.arguments.len() != parameters.len()
            || supplied.lifetime_arguments.len() != lifetime_parameters.len()
        {
            return Err(refuse(
                "native requirement application requires its complete exact member telescope",
            ));
        }
    }
    let retained_scope = has_outer
        .then(|| environment.retain(lower.program))
        .transpose()
        .map_err(refuse)?;
    if supplied.is_some() || (has_outer && parameters.is_empty()) {
        let mut supplied = supplied.unwrap_or(expr::StaticSymbolApplication {
            lifetime_arguments: Box::new([]),
            arguments: Box::new([]),
            member: Handle::invalid(),
        });
        let authored_arguments = supplied.arguments.clone();
        crate::monomorphization::normalize_const_arguments(
            lower.program,
            author,
            &mut supplied.arguments,
        )
        .map_err(|reason| refuse(&reason))?;
        let bindings = crate::monomorphization::requirement_applications::invocation_bindings(
            lower.program,
            &parameters,
            &supplied.arguments,
        )?;
        let lifetimes = lifetime_parameters
            .iter()
            .zip(supplied.lifetime_arguments.iter())
            .map(|(parameter, argument)| {
                typed_trees::static_applications::StaticMachineLifetimeBinding {
                    parameter: parameter.clone(),
                    argument: argument.clone(),
                }
            })
            .collect::<Vec<_>>();
        let application = lower
            .program
            .expression_table
            .retain_static_machine_application(requirement, name, &authored_arguments);
        let source = lower
            .program
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = author;
        source.binding_declaration = requirement;
        lower
            .program
            .expression_table
            .set_static_machine_application_bindings(application, &bindings);
        lower
            .program
            .expression_table
            .set_static_machine_application_lifetimes(application, &lifetimes);
        let instance = lower
            .program
            .instantiate_requirement_application(
                &environment,
                requirement,
                &trait_arguments,
                application,
            )
            .map_err(refuse)?;
        return Ok(PreparedProductRequirement {
            requirement,
            closed_requirement: Some(instance),
            trait_scope: retained_scope,
        });
    }
    Ok(PreparedProductRequirement {
        requirement,
        closed_requirement: None,
        trait_scope: retained_scope,
    })
}

struct Lower<'a> {
    program: &'a mut TypedTrees,
    graph: &'a ProductStaticOperandGraph,
    author: SymbolHandle,
    types: Vec<(Handle<ProductOperandType>, TypeReferenceHandle)>,
    expressions: Vec<(Handle<ProductOperandExpression>, expr::ExpressionHandle)>,
    subjects: Vec<(
        Handle<ProductMatchSubject>,
        Handle<expr::CompletedValueCapture>,
    )>,
    active_types: Vec<Handle<ProductOperandType>>,
    active_expressions: Vec<Handle<ProductOperandExpression>>,
    active_operands: Vec<Handle<ProductStaticOperand>>,
}
impl Lower<'_> {
    fn error(&self, reason: &str) -> Vec<Diagnostic> {
        vec![Diagnostic::error(reason)]
    }
    fn identifier(&self, name: Handle<ProductOperandName>) -> Identifier {
        Identifier::generated(&self.graph.names.get(name).text)
    }
    fn identifiers(&self, span: HandleSpan<ProductOperandName>) -> Vec<Identifier> {
        self.graph
            .names
            .span_or_empty(span)
            .iter()
            .map(|name| Identifier::generated(&name.text))
            .collect()
    }
    fn path(&self, span: HandleSpan<ProductOperandName>) -> Result<String, Vec<Diagnostic>> {
        let names = self.graph.names.span_or_empty(span);
        if names.is_empty() || names.len() != span.len() {
            return Err(self.error("designated operand has no complete source path"));
        }
        Ok(names
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>()
            .join("::"))
    }
    fn candidates(&self) -> Vec<(SymbolHandle, bool)> {
        let program = self.program.product_declarations();
        program
            .data_definitions()
            .iter()
            .map(|value| (value.symbol, value.is_public))
            .chain(program.data_definitions().iter().flat_map(|owner| {
                program
                    .data_members(owner)
                    .iter()
                    .flat_map(move |member| match member {
                        typed_trees::data::DataMember::Field(field) => {
                            vec![(field.symbol, owner.is_public)]
                        }
                        typed_trees::data::DataMember::Variant(case) => {
                            std::iter::once((case.symbol, owner.is_public))
                                .chain(
                                    program
                                        .data_payload_fields(case)
                                        .iter()
                                        .map(|field| (field.symbol, owner.is_public)),
                                )
                                .collect()
                        }
                    })
            }))
            .chain(
                program
                    .traits()
                    .iter()
                    .map(|value| (value.symbol, value.is_public)),
            )
            .chain(
                program
                    .conformances()
                    .iter()
                    .map(|value| (value.symbol, value.is_public)),
            )
            .chain(
                program
                    .const_declarations()
                    .iter()
                    .map(|value| (value.symbol, value.is_public)),
            )
            .chain(
                program
                    .domain_definitions()
                    .iter()
                    .map(|value| (value.symbol, value.is_public)),
            )
            .chain(
                program
                    .machines()
                    .iter()
                    .map(|value| (value.symbol, value.is_public)),
            )
            .chain(program.traits().iter().flat_map(|owner| {
                program
                    .trait_machine_signatures(owner)
                    .iter()
                    .map(move |value| (value.symbol, owner.is_public))
            }))
            .collect()
    }
    fn symbol(&self, name: Handle<ProductOperandName>) -> Result<SymbolHandle, Vec<Diagnostic>> {
        let name = self.graph.names.get(name);
        if let Some(atom) = name.builtin_type {
            return self
                .program
                .symbols
                .child_handles(self.program.symbols.root())
                .and_then(|mut children| {
                    children.find(|symbol| {
                        self.program.symbols.builtin_type_atom(*symbol) == Some(atom)
                    })
                })
                .ok_or_else(|| self.error("designated type lost its certified builtin atom"));
        }
        if let Some(function) = name.builtin_function {
            return self
                .program
                .symbols
                .builtin_function_symbol(function)
                .ok_or_else(|| {
                    self.error("designated expression lost its certified builtin function")
                });
        }
        // Inert Product operands may intentionally precede ordinary name
        // resolution. Rejoin compiler atoms with the ordinary source-scoped
        // type resolver, including data declarations so a same-spelled package
        // type cannot be reclassified as a builtin by its diagnostic text.
        if name.declaration.is_none()
            && let Some(symbol) = self
                .program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    &name.text,
                    &[symbols::SymbolKind::BuiltinType, symbols::SymbolKind::Data],
                    self.program
                        .symbols
                        .rejoin_product_occurrence(name.source_span),
                )
            && self.program.symbols.builtin_type_atom(symbol).is_some()
        {
            return Ok(symbol);
        }
        let candidates = self
            .candidates()
            .into_iter()
            .filter(|(symbol, _)| {
                name.declaration.is_none_or(|declaration| {
                    self.program.symbols.symbol_provenance_source_span(*symbol)
                        == Some(self.program.symbols.rejoin_product_occurrence(declaration))
                })
            })
            .collect::<Vec<_>>();
        self.program
            .symbols
            .find_build_subject_declaration_from_source(
                &name.text,
                name.source_span,
                candidates,
                &[],
            )
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(format!(
                        "designated static operand `{}` has no exact visible source declaration",
                        name.text
                    ))
                    .with_source_span(name.source_span),
                ]
            })
    }
    fn application_arguments(
        &mut self,
        handle: Handle<ProductStaticApplication>,
    ) -> Result<expr::StaticSymbolApplication, Vec<Diagnostic>> {
        let value = self.graph.applications.get(handle).clone();
        let arguments = self
            .graph
            .static_arguments
            .span_or_empty(value.arguments)
            .iter()
            .copied()
            .map(|value| self.operand(value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(expr::StaticSymbolApplication {
            arguments: arguments.into_boxed_slice(),
            lifetime_arguments: self.identifiers(value.lifetimes).into_boxed_slice(),
            member: Handle::invalid(),
        })
    }
    fn application(
        &mut self,
        handle: Handle<ProductStaticApplication>,
    ) -> Result<expr::StaticSymbolApplication, Vec<Diagnostic>> {
        let value = self.graph.applications.get(handle).clone();
        let arguments = self
            .graph
            .static_arguments
            .span_or_empty(value.arguments)
            .iter()
            .copied()
            .map(|value| self.operand(value))
            .collect::<Result<Vec<_>, _>>()?;
        let member = if value.member.is_valid() {
            let argument = self.operand(value.member)?;
            self.program.expression_table.insert_static_member(argument)
        } else {
            Handle::invalid()
        };
        Ok(expr::StaticSymbolApplication {
            arguments: arguments.into_boxed_slice(),
            lifetime_arguments: self.identifiers(value.lifetimes).into_boxed_slice(),
            member,
        })
    }
    fn operand(
        &mut self,
        handle: Handle<ProductStaticOperand>,
    ) -> Result<expr::StaticMachineArgument, Vec<Diagnostic>> {
        if self.active_operands.contains(&handle) {
            return Err(self.error("designated static operand graph contains a cycle"));
        }
        self.active_operands.push(handle);
        let value = self.graph.operands.get(handle).clone();
        let type_reference = self.ty_optional(value.structural_type)?;
        let application = value
            .application
            .is_valid()
            .then(|| self.application(value.application))
            .transpose()?
            .map(Box::new);
        let symbol = if let Some(last) = value.path.handles().last() {
            self.symbol(last)?
        } else {
            SymbolHandle::invalid()
        };
        let evidence_projection = value
            .projection
            .map(|(term, member)| expr::EvidenceProjection {
                term: self.identifier(term),
                member: self.identifier(member),
            });
        self.active_operands.pop();
        Ok(expr::StaticMachineArgument {
            type_reference,
            application,
            symbol,
            path: self.identifiers(value.path).into_boxed_slice(),
            const_literal: value.integer,
            evidence_projection,
        })
    }
    fn ty_optional(
        &mut self,
        handle: Handle<ProductOperandType>,
    ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
        if handle.is_valid() {
            self.ty(handle)
        } else {
            Ok(Handle::invalid())
        }
    }
    fn type_arguments(
        &mut self,
        span: HandleSpan<Handle<ProductOperandType>>,
    ) -> Result<HandleSpan<TypeReferenceHandle>, Vec<Diagnostic>> {
        let values = self
            .graph
            .type_arguments
            .span_or_empty(span)
            .iter()
            .copied()
            .map(|value| self.ty(value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self
            .program
            .type_reference_table
            .insert_type_reference_handles(values))
    }
    fn ty(
        &mut self,
        handle: Handle<ProductOperandType>,
    ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
        if let Some((_, target)) = self.types.iter().find(|(source, _)| *source == handle) {
            return Ok(*target);
        }
        if self.active_types.contains(&handle) {
            return Err(self.error("designated type graph contains a cycle"));
        }
        self.active_types.push(handle);
        let node = match self.graph.types.get(handle).clone() {
            ProductOperandType::Invalid => {
                return Err(self.error("designated type graph contains an invalid node"));
            }
            ProductOperandType::Unit => TypeReferenceNode::Unit,
            ProductOperandType::Named(name) => TypeReferenceNode::Named {
                symbol: self.symbol(name)?,
                name: self.identifier(name),
            },
            ProductOperandType::Generic {
                name,
                lifetimes,
                arguments,
            } => TypeReferenceNode::Generic {
                base_symbol: self.symbol(name)?,
                base_name: self.identifier(name),
                lifetime_arguments: self.identifiers(lifetimes),
                arguments: self.type_arguments(arguments)?,
            },
            ProductOperandType::Dynamic { name, arguments } => TypeReferenceNode::DynamicTrait {
                symbol: self.symbol(name)?,
                name: self.identifier(name),
                arguments: self.type_arguments(arguments)?,
            },
            ProductOperandType::Reference {
                referee,
                access,
                lifetime,
            } => TypeReferenceNode::Reference {
                referee: self.ty(referee)?,
                access,
                lifetime: lifetime.map(|name| self.identifier(name)),
            },
            ProductOperandType::Array { element, length } => TypeReferenceNode::FixedArray {
                element_type: self.ty(element)?,
                length: match length {
                    ProductArrayLength::Literal(value) => types::FixedArrayLength::Literal(value),
                    ProductArrayLength::Expression(value) => {
                        types::FixedArrayLength::Expression(self.ty(value)?)
                    }
                    ProductArrayLength::Parameter(name) => {
                        types::FixedArrayLength::ConstParameter {
                            symbol: self.symbol(name)?,
                            name: self.identifier(name),
                        }
                    }
                    ProductArrayLength::Call(name) => types::FixedArrayLength::ConstCall {
                        machine: self.symbol(name)?,
                        name: self.identifier(name),
                        source_span: self.graph.names.get(name).source_span,
                    },
                },
            },
            ProductOperandType::Slice(element) => TypeReferenceNode::Slice {
                element_type: self.ty(element)?,
            },
            ProductOperandType::ConstExpression(expression) => {
                TypeReferenceNode::ConstExpression(self.expression(expression)?)
            }
            ProductOperandType::Constrained { base, constraints } => {
                let base_type = self.ty(base)?;
                let values = self
                    .graph
                    .constraints
                    .span_or_empty(constraints)
                    .iter()
                    .cloned()
                    .map(|constraint| self.constraint(constraint))
                    .collect::<Result<Vec<_>, _>>()?;
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints: self.program.type_reference_table.insert_constraints(values),
                }
            }
        };
        let target = self.program.type_reference_table.insert(node);
        self.program
            .type_reference_table
            .retain_operand_lifetime_owner(target, self.author)
            .map_err(|reason| self.error(reason))?;
        self.active_types.pop();
        self.types.push((handle, target));
        Ok(target)
    }
    fn constraint(
        &mut self,
        constraint: ProductOperandConstraint,
    ) -> Result<types::TypeConstraintNode, Vec<Diagnostic>> {
        Ok(match constraint {
            ProductOperandConstraint::Invalid => {
                return Err(self.error("designated constraint graph contains an invalid node"));
            }
            ProductOperandConstraint::Named(name) => {
                types::TypeConstraintNode::Named(self.identifier(name))
            }
            ProductOperandConstraint::Arithmetic(policy) => {
                types::TypeConstraintNode::ArithmeticDomain(policy)
            }
            ProductOperandConstraint::Range {
                minimum,
                maximum,
                inclusive,
            } => types::TypeConstraintNode::Range {
                minimum: self.expression(minimum)?,
                maximum: self.expression(maximum)?,
                end_inclusive: inclusive,
            },
            ProductOperandConstraint::Domain { name, arguments } => {
                let span = self.type_arguments(arguments)?;
                types::TypeConstraintNode::Domain(types::DomainConstraint { name: self.identifier(name), symbol: self.symbol(name)?, arguments: self.program.type_reference_table.type_reference_handles(span).to_vec(), authored_selection: Some(types::AuthoredDomainConstraintSelection { source_span: self.graph.names.get(name).source_span, exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation }), ..Default::default() })
            }
        })
    }
}
