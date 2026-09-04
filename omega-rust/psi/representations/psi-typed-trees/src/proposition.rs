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
    pub is_public: bool,
    pub binders: HandleSpan<PropositionBinder>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    /// Exact authored expression occurrence for a transparent formula. This
    /// is review custody, not normalized proposition identity.
    pub transparent_formula_source_span: Option<psi_source::SourceSpan>,
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
    pub kind: PropositionBinderArgumentKind,
    pub path: Box<[Identifier]>,
    pub const_literal: Option<psi_numerics::literals::IntegerLiteral>,
    pub evidence_projection: Option<crate::expression::EvidenceProjection>,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}

impl PropositionBinderArgument {
    pub fn display_name(&self) -> String {
        display_binder_argument(self)
    }
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
    Witness {
        evidence: String,
        interface: Option<NormalizedEvidenceInterfaceIdentity>,
    },
}

fn normalize_evidence_interface(
    program: &crate::TypedTrees,
    evidence: TypeReferenceHandle,
    binders: &[PropositionBinder],
    binder_labels: &[String],
    binder_identities: &[Option<String>],
) -> Option<(String, Option<NormalizedEvidenceInterfaceIdentity>)> {
    let display_substitutions = binders
        .iter()
        .zip(binder_labels)
        .map(|(binder, replacement)| (binder.symbol, replacement.clone()))
        .collect::<Vec<_>>();
    let label = render_evidence_type(program, evidence, &display_substitutions);
    let (trait_symbol, arguments) = match program.type_reference_table.type_reference(evidence) {
        crate::types::TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..]),
        crate::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => (
            *base_symbol,
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        ),
        _ => return Some((label, None)),
    };
    if !trait_symbol.is_valid()
        || program.symbols.get(trait_symbol).kind != psi_symbols::SymbolKind::Trait
    {
        return Some((label, None));
    }
    let unresolved_binders = binders
        .iter()
        .zip(binder_identities)
        .filter_map(|(binder, identity)| identity.is_none().then_some(binder.symbol))
        .collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| type_reference_mentions_any(program, *argument, &unresolved_binders))
    {
        return Some((label, None));
    }
    let exact_substitutions = binders
        .iter()
        .zip(binder_identities)
        .filter_map(|(binder, replacement)| replacement.clone().map(|value| (binder.symbol, value)))
        .collect::<Vec<_>>();
    let arguments = arguments
        .iter()
        .map(|argument| {
            program.normalized_type_identity_with_binders(*argument, &exact_substitutions)
        })
        .collect::<Vec<_>>();
    let root_arguments = arguments
        .iter()
        .map(|argument| argument.as_str().to_owned())
        .collect::<Vec<_>>();
    Some((
        label,
        Some(NormalizedEvidenceInterfaceIdentity {
            trait_symbol,
            arguments,
            requirements: normalized_evidence_requirements(program, trait_symbol, &root_arguments),
        }),
    ))
}

fn normalized_evidence_requirements(
    program: &crate::TypedTrees,
    root: SymbolHandle,
    root_arguments: &[String],
) -> Vec<NormalizedEvidenceRequirementIdentity> {
    fn collect(
        program: &crate::TypedTrees,
        trait_symbol: SymbolHandle,
        trait_arguments: &[String],
        visited: &mut Vec<(SymbolHandle, Vec<String>)>,
        requirements: &mut Vec<NormalizedEvidenceRequirementIdentity>,
    ) {
        let visit = (trait_symbol, trait_arguments.to_vec());
        if visited.contains(&visit) {
            return;
        }
        visited.push(visit);
        let Some(definition) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == trait_symbol)
        else {
            return;
        };
        for requirement in program.trait_machine_signatures(definition) {
            if !requirements.iter().any(|candidate| {
                candidate.declaring_trait == definition.symbol
                    && candidate.declaring_trait_arguments == trait_arguments
                    && candidate.requirement == requirement.symbol
            }) {
                requirements.push(NormalizedEvidenceRequirementIdentity {
                    declaring_trait: definition.symbol,
                    declaring_trait_arguments: trait_arguments.to_vec(),
                    requirement: requirement.symbol,
                });
            }
        }
        let substitutions = program
            .trait_type_parameters(definition)
            .iter()
            .zip(trait_arguments)
            .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
            .collect::<Vec<_>>();
        for parent in program.trait_requirements(definition) {
            let parent_arguments = program
                .type_reference_table
                .type_reference_handles(parent.arguments)
                .iter()
                .map(|argument| {
                    program
                        .normalized_type_identity_with_binders(*argument, &substitutions)
                        .into_string()
                })
                .collect::<Vec<_>>();
            collect(
                program,
                parent.symbol,
                &parent_arguments,
                visited,
                requirements,
            );
        }
    }

    let mut requirements = Vec::new();
    collect(
        program,
        root,
        root_arguments,
        &mut Vec::new(),
        &mut requirements,
    );
    requirements.sort_by_key(|requirement| {
        (
            requirement.declaring_trait.arena_index(),
            requirement.requirement.arena_index(),
            requirement.declaring_trait_arguments.clone(),
        )
    });
    requirements
}

fn type_reference_mentions_any(
    program: &crate::TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &[SymbolHandle],
) -> bool {
    use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_mentions_any(program, *referee, symbols)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_reference_mentions_any(program, *base_type, symbols)
                || program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Range { minimum, maximum } => {
                            expression_mentions_any(program, *minimum, symbols)
                                || expression_mentions_any(program, *maximum, symbols)
                        }
                        TypeConstraintNode::Domain(domain) => {
                            domain.arguments.iter().any(|argument| {
                                type_reference_mentions_any(program, *argument, symbols)
                            })
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            false
                        }
                    })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            type_reference_mentions_any(program, *element_type, symbols)
                || matches!(
                    length,
                    FixedArrayLength::ConstParameter { symbol, .. }
                        if symbols.contains(symbol)
                )
        }
        TypeReferenceNode::Slice { element_type } => {
            type_reference_mentions_any(program, *element_type, symbols)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_reference_mentions_any(program, *argument, symbols)),
        TypeReferenceNode::ConstExpression(expression) => {
            expression_mentions_any(program, *expression, symbols)
        }
        TypeReferenceNode::Named { symbol, .. } => symbols.contains(symbol),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Unit => false,
    }
}

fn expression_mentions_any(
    program: &crate::TypedTrees,
    expression: ExpressionHandle,
    symbols: &[SymbolHandle],
) -> bool {
    use crate::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => symbols.contains(&path.symbol),
        ExpressionNode::Binary(binary) => {
            expression_mentions_any(program, binary.left, symbols)
                || expression_mentions_any(program, binary.right, symbols)
        }
        ExpressionNode::Unary(unary) => expression_mentions_any(program, unary.operand, symbols),
        ExpressionNode::Borrow(inner) => expression_mentions_any(program, inner.target, symbols),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => false,
        // These shapes are not admitted in proof-static type arguments. Keep
        // an unresolved binder fail-closed if one survives into such a shape.
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => !symbols.is_empty(),
    }
}

fn exact_binder_argument_identity(
    program: &crate::TypedTrees,
    argument: &PropositionBinderArgument,
) -> Option<String> {
    if argument.const_literal.is_some() || !argument.symbol.is_valid() {
        return None;
    }
    if matches!(
        program.symbols.get(argument.symbol).kind,
        psi_symbols::SymbolKind::TypeParameter
            | psi_symbols::SymbolKind::MachineParameter
            | psi_symbols::SymbolKind::PropositionParameter
            | psi_symbols::SymbolKind::PropositionMachineParameter
    ) {
        return None;
    }
    let identity = program.symbols.display_path(argument.symbol, "::");
    (!identity.is_empty()).then_some(identity)
}

fn binder_argument_identities_for_labels(
    program: &crate::TypedTrees,
    application: &PropositionApplication,
    binder_labels: &[String],
) -> Option<Vec<Option<String>>> {
    if application.binder_arguments.len() == binder_labels.len() {
        return Some(
            application
                .binder_arguments
                .iter()
                .map(|argument| exact_binder_argument_identity(program, argument))
                .collect(),
        );
    }
    // A proposition-family law has no concrete binder arguments until the
    // selected family is substituted. Validation synthesizes its labels from
    // the satisfier's representative telescope. Those labels remain
    // intentionally unresolved semantic identities, but their arity must
    // still reach primitive/fact-only normalization.
    application
        .binder_arguments
        .is_empty()
        .then(|| vec![None; binder_labels.len()])
}

fn render_evidence_type(
    program: &crate::TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, String)],
) -> String {
    use crate::types::TypeReferenceNode;

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => format!(
            "&{}{}",
            match access {
                psi_language_core::ReferenceAccess::Shared => "",
                psi_language_core::ReferenceAccess::Mutable => "mut ",
                psi_language_core::ReferenceAccess::WriteOnly => "write ",
            },
            render_evidence_type(program, *referee, substitutions)
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => format!(
            "{}[{}]",
            render_evidence_type(program, *base_type, substitutions),
            match constraints.count() {
                1 => "1 constraint".to_owned(),
                count => format!("{count} constraints"),
            }
        ),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => format!(
            "[{}; {length}]",
            render_evidence_type(program, *element_type, substitutions)
        ),
        TypeReferenceNode::Slice { element_type } => format!(
            "[{}]",
            render_evidence_type(program, *element_type, substitutions)
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => format!(
            "{base_name}<{}>",
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| render_evidence_type(program, *argument, substitutions))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeReferenceNode::Named { symbol, name } => substitutions
            .iter()
            .find(|(candidate, _)| candidate == symbol)
            .map(|(_, replacement)| replacement.clone())
            .unwrap_or_else(|| name.to_string()),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => program.display_type_reference(type_reference),
    }
}

/// Exact instantiated identity of one carrierless evidence interface. Display
/// spelling remains diagnostic; selection compares the resolved trait symbol
/// and canonical argument identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEvidenceInterfaceIdentity {
    pub trait_symbol: SymbolHandle,
    pub arguments: Vec<crate::type_identity::NormalizedTypeIdentity>,
    /// Complete direct and inherited proof-static requirement surface.
    pub requirements: Vec<NormalizedEvidenceRequirementIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEvidenceRequirementIdentity {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement: SymbolHandle,
}

/// Structured endpoint of transparent-alias expansion for consumers that
/// need a self-contained proposition application rather than its display
/// label. Generic proposition parameters have no nominal endpoint until
/// specialization and therefore do not produce this record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPropositionApplicationIdentity {
    pub declaration: SymbolHandle,
    pub name: String,
    pub binder_arguments: Vec<NormalizedPropositionBinderArgument>,
    pub arguments: Vec<String>,
    pub classification: PropositionEvidenceClassification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPropositionBinderArgument {
    pub kind: PropositionBinderArgumentKind,
    pub identity: String,
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
                classification: PropositionEvidenceClassification::Witness { evidence, .. },
            } => format!("proposition:witness:{evidence}:{label}"),
            Self::Boolean { label } => format!("boolean:{label}"),
        }
    }
}

impl crate::TypedTrees {
    pub fn normalize_nominal_proposition_application(
        &self,
        application: &PropositionApplication,
    ) -> Option<NormalizedPropositionApplicationIdentity> {
        let binder_labels = application
            .binder_arguments
            .iter()
            .map(display_binder_argument)
            .collect::<Vec<_>>();
        let argument_labels = self
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .map(|argument| self.expression_table.display_name(*argument))
            .collect::<Vec<_>>();
        self.normalize_nominal_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
    }

    pub fn normalize_nominal_proposition_application_with_labels(
        &self,
        application: &PropositionApplication,
        binder_labels: &[String],
        argument_labels: &[String],
    ) -> Option<NormalizedPropositionApplicationIdentity> {
        let binder_identities =
            binder_argument_identities_for_labels(self, application, binder_labels)?;
        self.normalize_nominal_proposition_application_inner(
            application,
            binder_labels,
            argument_labels,
            &binder_identities,
            &mut Vec::new(),
        )
    }

    fn normalize_nominal_proposition_application_inner(
        &self,
        application: &PropositionApplication,
        binder_labels: &[String],
        argument_labels: &[String],
        binder_identities: &[Option<String>],
        visiting: &mut Vec<SymbolHandle>,
    ) -> Option<NormalizedPropositionApplicationIdentity> {
        if visiting.contains(&application.proposition) {
            return None;
        }
        let declaration = self
            .propositions()
            .iter()
            .find(|candidate| candidate.symbol == application.proposition)?;
        let binders = self.proposition_binders(declaration);
        let parameters = self.proposition_parameters(declaration);
        if binders.len() != binder_labels.len()
            || binders.len() != binder_identities.len()
            || parameters.len() != argument_labels.len()
        {
            return None;
        }
        let substitutions = binders
            .iter()
            .zip(binder_labels)
            .map(|(binder, label)| (binder.symbol, label.clone()))
            .chain(
                parameters
                    .iter()
                    .zip(argument_labels)
                    .map(|(parameter, label)| (parameter.symbol, label.clone())),
            )
            .collect::<Vec<_>>();
        let endpoint = |classification| NormalizedPropositionApplicationIdentity {
            declaration: declaration.symbol,
            name: declaration.name.as_str().to_owned(),
            binder_arguments: binders
                .iter()
                .zip(binder_labels)
                .map(|(binder, identity)| NormalizedPropositionBinderArgument {
                    kind: match binder.kind {
                        PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                        PropositionBinderKind::Const { .. } => PropositionBinderArgumentKind::Const,
                        PropositionBinderKind::Machine => PropositionBinderArgumentKind::Machine,
                    },
                    identity: identity.clone(),
                })
                .collect(),
            arguments: argument_labels.to_vec(),
            classification,
        };
        match &declaration.body {
            PropositionBody::Primitive => {
                Some(endpoint(PropositionEvidenceClassification::FactOnly))
            }
            PropositionBody::Witness { evidence } => {
                let (evidence, interface) = normalize_evidence_interface(
                    self,
                    *evidence,
                    binders,
                    binder_labels,
                    binder_identities,
                )?;
                Some(endpoint(PropositionEvidenceClassification::Witness {
                    evidence,
                    interface,
                }))
            }
            PropositionBody::Transparent {
                proposition: PropositionFormula::BooleanExpression(_),
            } => None,
            PropositionBody::Transparent {
                proposition: PropositionFormula::Application(expansion),
            } => {
                visiting.push(application.proposition);
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
                let expanded_binder_identities = expansion
                    .binder_arguments
                    .iter()
                    .map(|argument| {
                        binders
                            .iter()
                            .zip(binder_identities)
                            .find(|(binder, _)| binder.symbol == argument.symbol)
                            .and_then(|(_, identity)| identity.clone())
                            .or_else(|| exact_binder_argument_identity(self, argument))
                    })
                    .collect::<Vec<_>>();
                let expanded_arguments = self
                    .expression_table
                    .expression_handles(expansion.arguments)
                    .iter()
                    .map(|argument| render_expression(self, *argument, &substitutions, &[]))
                    .collect::<Vec<_>>();
                let normalized = self.normalize_nominal_proposition_application_inner(
                    expansion,
                    &expanded_binders,
                    &expanded_arguments,
                    &expanded_binder_identities,
                    visiting,
                );
                visiting.pop();
                normalized
            }
        }
    }

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
            .map(display_binder_argument)
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
        let binder_identities =
            binder_argument_identities_for_labels(self, application, binder_labels)?;
        let mut visiting = Vec::new();
        self.normalize_proposition_application_inner(
            application,
            binder_labels,
            argument_labels,
            &binder_identities,
            &mut visiting,
        )
    }

    fn normalize_proposition_application_inner(
        &self,
        application: &PropositionApplication,
        binder_labels: &[String],
        argument_labels: &[String],
        binder_identities: &[Option<String>],
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
        if binders.len() != binder_labels.len()
            || binders.len() != binder_identities.len()
            || parameters.len() != argument_labels.len()
        {
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
                let (evidence, interface) = normalize_evidence_interface(
                    self,
                    *evidence,
                    binders,
                    binder_labels,
                    binder_identities,
                )?;
                Some(NormalizedPropositionFormula::Proposition {
                    label: canonical_application_label(
                        declaration.name.as_str(),
                        binder_labels,
                        argument_labels,
                    ),
                    classification: PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
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
                        let expanded_binder_identities = expansion
                            .binder_arguments
                            .iter()
                            .map(|argument| {
                                binders
                                    .iter()
                                    .zip(binder_identities)
                                    .find(|(binder, _)| binder.symbol == argument.symbol)
                                    .and_then(|(_, identity)| identity.clone())
                                    .or_else(|| exact_binder_argument_identity(self, argument))
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
                            &expanded_binder_identities,
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
    if let Some(literal) = &argument.const_literal {
        return literal.text().to_owned();
    }
    if let Some(projection) = &argument.evidence_projection {
        return format!("{}.{}", projection.term, projection.member);
    }
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
        ExpressionNode::Borrow(inner) => format!(
            "{} {}",
            match inner.access {
                psi_language_core::ReferenceAccess::Mutable => "mut",
                psi_language_core::ReferenceAccess::WriteOnly => "write",
                psi_language_core::ReferenceAccess::Shared => "shared",
            },
            render(inner.target)
        ),
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
