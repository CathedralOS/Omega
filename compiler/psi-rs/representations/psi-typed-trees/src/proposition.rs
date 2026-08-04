use crate::data::DataProperties;
use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

/// A typed proof-formula declaration. It remains outside the executable
/// machine graph and owns no runtime result or body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub binders: HandleSpan<PropositionBinder>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub body: PropositionBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionBinder {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub kind: PropositionBinderKind,
    pub bounds: DataProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBinderKind {
    #[default]
    Type,
    Const {
        type_reference: TypeReferenceHandle,
    },
    Machine,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PropositionBody {
    #[default]
    Primitive,
    Witness {
        evidence: TypeReferenceHandle,
    },
    /// Source/debug expansion only; normalized proof facts inline this before
    /// semantic identity is minted.
    Transparent {
        proposition: PropositionFormula,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionFormula {
    Application(PropositionApplication),
    BooleanExpression(ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionApplication {
    pub proposition: SymbolHandle,
    pub name: Identifier,
    pub binder_arguments: Box<[PropositionBinderArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionBinderArgument {
    pub path: Box<[Identifier]>,
    pub symbol: SymbolHandle,
}

/// The proof formula after transparent proposition aliases have been expanded.
/// The canonical label is deliberately source-name-free for transparent aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedPropositionFormula {
    Proposition {
        label: String,
        classification: PropositionEvidenceClassification,
    },
    Boolean {
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionEvidenceClassification {
    FactOnly,
    Witness { evidence: String },
}

impl NormalizedPropositionFormula {
    pub fn identity_label(&self) -> String {
        match self {
            Self::Proposition {
                label,
                classification: PropositionEvidenceClassification::FactOnly,
            } => format!("proposition:fact:{label}"),
            Self::Proposition {
                label,
                classification: PropositionEvidenceClassification::Witness { evidence },
            } => format!("proposition:witness:{evidence}:{label}"),
            Self::Boolean { label } => format!("boolean:{label}"),
        }
    }
}

impl crate::TypedTrees {
    pub fn render_proof_expression_with_symbols(
        &self,
        expression: ExpressionHandle,
        substitutions: &[(SymbolHandle, String)],
    ) -> String {
        render_expression(self, expression, substitutions, &[])
    }

    pub fn render_proof_expression_with_parameters(
        &self,
        expression: ExpressionHandle,
        substitutions: &[(SymbolHandle, String, String)],
    ) -> String {
        let symbol_substitutions = substitutions
            .iter()
            .map(|(symbol, _, replacement)| (*symbol, replacement.clone()))
            .collect::<Vec<_>>();
        let name_substitutions = substitutions
            .iter()
            .map(|(_, name, replacement)| (name.clone(), replacement.clone()))
            .collect::<Vec<_>>();
        render_expression(self, expression, &symbol_substitutions, &name_substitutions)
    }

    pub fn normalize_proposition_application(
        &self,
        application: &PropositionApplication,
    ) -> Option<NormalizedPropositionFormula> {
        let binder_labels = application
            .binder_arguments
            .iter()
            .map(|argument| display_binder_argument(argument))
            .collect::<Vec<_>>();
        let argument_labels = self
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .map(|argument| self.expression_table.display_name(*argument))
            .collect::<Vec<_>>();
        self.normalize_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
    }

    pub fn normalize_proposition_application_with_labels(
        &self,
        application: &PropositionApplication,
        binder_labels: &[String],
        argument_labels: &[String],
    ) -> Option<NormalizedPropositionFormula> {
        let mut visiting = Vec::new();
        self.normalize_proposition_application_inner(
            application,
            binder_labels,
            argument_labels,
            &mut visiting,
        )
    }

    fn normalize_proposition_application_inner(
        &self,
        application: &PropositionApplication,
        binder_labels: &[String],
        argument_labels: &[String],
        visiting: &mut Vec<SymbolHandle>,
    ) -> Option<NormalizedPropositionFormula> {
        if visiting.contains(&application.proposition) {
            return None;
        }
        let declaration = self
            .propositions()
            .iter()
            .find(|candidate| candidate.symbol == application.proposition);
        if declaration.is_none()
            && self.symbols.get(application.proposition).kind
                == psi_symbols::SymbolKind::PropositionParameter
        {
            let parameter_count = self
                .data_type_parameters
                .iter()
                .map(|(_, parameter)| parameter)
                .find_map(|parameter| match &parameter.kind {
                    crate::data::TypeParameterKind::Proposition { contract }
                        if parameter.symbol == application.proposition =>
                    {
                        Some(contract.parameters.len())
                    }
                    _ => None,
                })?;
            if !binder_labels.is_empty() || parameter_count != argument_labels.len() {
                return None;
            }
            return Some(NormalizedPropositionFormula::Proposition {
                label: canonical_application_label(
                    application.name.as_str(),
                    binder_labels,
                    argument_labels,
                ),
                classification: PropositionEvidenceClassification::FactOnly,
            });
        }
        let declaration = declaration?;
        let binders = self.proposition_binders(declaration);
        let parameters = self.proposition_parameters(declaration);
        if binders.len() != binder_labels.len() || parameters.len() != argument_labels.len() {
            return None;
        }

        let mut substitutions = Vec::new();
        for (binder, label) in binders.iter().zip(binder_labels) {
            substitutions.push((binder.symbol, label.clone()));
        }
        for (parameter, label) in parameters.iter().zip(argument_labels) {
            substitutions.push((parameter.symbol, label.clone()));
        }

        match &declaration.body {
            PropositionBody::Primitive => Some(NormalizedPropositionFormula::Proposition {
                label: canonical_application_label(
                    declaration.name.as_str(),
                    binder_labels,
                    argument_labels,
                ),
                classification: PropositionEvidenceClassification::FactOnly,
            }),
            PropositionBody::Witness { evidence } => {
                Some(NormalizedPropositionFormula::Proposition {
                    label: canonical_application_label(
                        declaration.name.as_str(),
                        binder_labels,
                        argument_labels,
                    ),
                    classification: PropositionEvidenceClassification::Witness {
                        evidence: self.display_type_reference(*evidence),
                    },
                })
            }
            PropositionBody::Transparent { proposition } => {
                visiting.push(application.proposition);
                let normalized = match proposition {
                    PropositionFormula::BooleanExpression(expression) => {
                        Some(NormalizedPropositionFormula::Boolean {
                            label: render_expression(self, *expression, &substitutions, &[]),
                        })
                    }
                    PropositionFormula::Application(expansion) => {
                        let expanded_binders = expansion
                            .binder_arguments
                            .iter()
                            .map(|argument| {
                                substitutions
                                    .iter()
                                    .find(|(symbol, _)| *symbol == argument.symbol)
                                    .map(|(_, label)| label.clone())
                                    .unwrap_or_else(|| display_binder_argument(argument))
                            })
                            .collect::<Vec<_>>();
                        let expanded_arguments = self
                            .expression_table
                            .expression_handles(expansion.arguments)
                            .iter()
                            .map(|argument| render_expression(self, *argument, &substitutions, &[]))
                            .collect::<Vec<_>>();
                        self.normalize_proposition_application_inner(
                            expansion,
                            &expanded_binders,
                            &expanded_arguments,
                            visiting,
                        )
                    }
                };
                visiting.pop();
                normalized
            }
        }
    }
}

fn canonical_application_label(name: &str, binders: &[String], arguments: &[String]) -> String {
    let binder_suffix = if binders.is_empty() {
        String::new()
    } else {
        format!("<{}>", binders.join(","))
    };
    format!("{name}{binder_suffix}({})", arguments.join(","))
}

fn display_binder_argument(argument: &PropositionBinderArgument) -> String {
    argument
        .path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn render_expression(
    program: &crate::TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, String)],
    name_substitutions: &[(String, String)],
) -> String {
    use crate::expression::ExpressionNode;
    let render =
        |expression| render_expression(program, expression, substitutions, name_substitutions);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            format!("atomic[{:?}]({})", atomic.ordering, render(atomic.value))
        }
        ExpressionNode::ArrayLiteral(values) => format!(
            "[{}]",
            program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| render(*value))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            render(binary.left),
            binary.operator.display_name(),
            render(binary.right)
        ),
        ExpressionNode::Boolean(value) => value.to_string(),
        ExpressionNode::Cast(cast) => format!(
            "({} as {})",
            render(cast.value),
            crate::expression::display_name_path(
                program
                    .expression_table
                    .name_path_members(cast.target_label),
                "::",
            )
        ),
        ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| render(*argument))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!("{}.{}({arguments})", render(call.receiver), call.target)
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        ExpressionNode::Float(value) => value.to_string(),
        ExpressionNode::Indexed(indexed) => {
            format!("{}[{}]", render(indexed.collection), render(indexed.index))
        }
        ExpressionNode::Integer(value) => value.to_string(),
        ExpressionNode::Member(member) => format!("{}.{}", render(member.receiver), member.member),
        ExpressionNode::Mutable(inner) => format!("mut {}", render(*inner)),
        ExpressionNode::Name(path) => {
            if let Some((_, replacement)) = substitutions
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol || *symbol == path.head_symbol)
            {
                return replacement.clone();
            }
            let members = program.expression_table.name_path_members(path.members);
            if let Some(first) = members.first()
                && let Some((_, replacement)) = name_substitutions
                    .iter()
                    .find(|(name, _)| name == first.as_str())
            {
                return replacement.clone();
            }
            crate::expression::display_name_path(members, "::")
        }
        ExpressionNode::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
            (true, true) => format!("{}..{}", render(range.start), render(range.end)),
            (true, false) => format!("{}..", render(range.start)),
            (false, true) => format!("..{}", render(range.end)),
            (false, false) => "..".to_owned(),
        },
        ExpressionNode::String(value) => format!("{value:?}"),
        ExpressionNode::StructLiteral(_) | ExpressionNode::ZeroValue(_) => {
            program.expression_table.display_name(expression)
        }
        ExpressionNode::Unary(unary) => {
            format!("{}{}", unary.operator.display_name(), render(unary.operand))
        }
    }
}
