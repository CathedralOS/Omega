//! Source descriptions for the checked indexed W scheme. Mathematical
//! encodings do not replace source nominal identity, field custody or native
//! layout: source-domain aliases keep the declaration head opaque to those
//! checks while the kernel checks its defined W body.
//!
//! Every constructor label stores its common fields, exact active payload and
//! complete predicates. Recursive fields become ordered child positions whose
//! next indices are closed functions of that same source telescope. This file
//! does not turn legacy constructor-success verdicts into evidence: actual
//! constructor applications additionally need their retained derivations.
//!
//! Ordinary source Type binders denote Type0 carriers, not scalar tags. They
//! are retained in the dependent index and label telescope, so `marker: T`
//! and a recursive child's Type actual remain tied to the same declaration.
//! That telescope itself inhabits Type1; child positions and predicate proofs
//! remain at their original levels. Every generated level and application is
//! still checked by the kernel without cumulativity or a new admission rule.
//! Higher-universe, const/machine and region-bearing source descriptions are
//! not silently projected into this ordinary Type/Value fragment.

mod construction;
pub(super) use construction::SourceConstructorContext;
mod induction;
mod terms;

use super::{Elaborator, ScalarCarrier};
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level, Signature, Sort,
    Term, TermHandle,
};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter, TypeParameterKind};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Decide the fixed prefix before any source declaration is assigned a
/// position. Programs with no source indexed application keep their original
/// signature and serialized declaration coordinates.
pub(super) fn needs_scheme(program: &dyn validation::CheckingInput) -> bool {
    fn reference(program: &dyn validation::CheckingInput, handle: TypeReferenceHandle) -> bool {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                program
                    .unique_data_definition_by_symbol(*base_symbol)
                    .is_some_and(|data| {
                        let parameters = program.data_type_parameters(data);
                        !parameters.is_empty()
                            && parameters.iter().all(|p| {
                                matches!(
                                    p.kind,
                                    TypeParameterKind::Type | TypeParameterKind::Value { .. }
                                )
                            })
                    })
                    || program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .any(|argument| reference(program, *argument))
            }
            TypeReferenceNode::Reference { referee, .. } => reference(program, *referee),
            TypeReferenceNode::Constrained { base_type, .. } => reference(program, *base_type),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => reference(program, *element_type),
            _ => false,
        }
    }
    fn mathematical(
        program: &dyn validation::CheckingInput,
        handle: typed_trees::mathematical::MathematicalTypeHandle,
    ) -> bool {
        use typed_trees::mathematical::MathematicalType;
        match program.mathematical_type(handle) {
            MathematicalType::Ordinary(handle) => reference(program, *handle),
            MathematicalType::Arrow {
                domain, codomain, ..
            } => mathematical(program, *domain) || mathematical(program, *codomain),
            MathematicalType::Application { callee, .. } => mathematical(program, *callee),
        }
    }
    !erased_family_demands(program).is_empty()
        || program.mathematical_definitions().iter().any(|definition| {
            mathematical(program, definition.result)
                || program
                    .mathematical_parameters(definition.parameters)
                    .iter()
                    .any(|parameter| mathematical(program, parameter.ty))
                || program
                    .data_type_parameters
                    .span_or_empty(definition.binders)
                    .iter()
                    .any(|parameter| match parameter.kind {
                        TypeParameterKind::Value { type_reference }
                        | TypeParameterKind::Const { type_reference } => {
                            reference(program, type_reference)
                        }
                        _ => false,
                    })
        })
        || validation::mathematical_applications::expression_nodes(program)
            .iter()
            .any(|expression| {
                validation::mathematical_applications::operand_type_reference(program, *expression)
                    .is_some_and(|handle| reference(program, handle))
            })
}

/// Exact family declarations demanded by erased source bindings. This scan
/// does not inspect names or treat an arbitrary relevant runtime generic as a
/// mathematical carrier. It also covers constructor-only source programs.
fn erased_family_demands(program: &dyn validation::CheckingInput) -> Vec<SymbolHandle> {
    fn demand(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
        result: &mut Vec<SymbolHandle>,
    ) {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                if program
                    .unique_data_definition_by_symbol(*base_symbol)
                    .is_some_and(|data| {
                        let parameters = program.data_type_parameters(data);
                        !parameters.is_empty()
                            && parameters.iter().all(|parameter| {
                                matches!(
                                    parameter.kind,
                                    TypeParameterKind::Type | TypeParameterKind::Value { .. }
                                )
                            })
                            && program.data_members(data).iter().any(|member| {
                                let fields = match member {
                                    DataMember::Field(field) => std::slice::from_ref(field),
                                    DataMember::Variant(case) => program.data_payload_fields(case),
                                };
                                fields.iter().any(|field| {
                                    declaration_contains_family(
                                        program,
                                        data.symbol,
                                        field.type_reference,
                                        &mut Vec::new(),
                                        &mut Vec::new(),
                                    )
                                })
                            })
                    })
                    && !result.contains(base_symbol)
                {
                    result.push(*base_symbol);
                }
                for &argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    demand(program, argument, result);
                }
            }
            TypeReferenceNode::Reference { referee, .. } => demand(program, *referee, result),
            TypeReferenceNode::Constrained { base_type, .. } => demand(program, *base_type, result),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => demand(program, *element_type, result),
            _ => {}
        }
    }
    let mut result = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.relevance == language_core::BindingRelevance::Erased {
                    demand(program, parameter.type_reference, &mut result);
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::LocalData(local) = statement {
                    if local.relevance == language_core::BindingRelevance::Erased {
                        demand(program, local.type_reference, &mut result);
                    }
                }
            }
        }
    }
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(case) => program.data_payload_fields(case),
            };
            for field in fields {
                if field.relevance == language_core::BindingRelevance::Erased {
                    demand(program, field.type_reference, &mut result);
                }
            }
        }
    }
    result
}

/// The exact source declaration graph used for both recursion admission and
/// erased-only evidence demand; ordinary nonrecursive generic records keep
/// their existing erased-field/array checking path.
fn declaration_contains_family(
    program: &dyn validation::CheckingInput,
    owner: SymbolHandle,
    reference: TypeReferenceHandle,
    references: &mut Vec<TypeReferenceHandle>,
    declarations: &mut Vec<SymbolHandle>,
) -> bool {
    if references.contains(&reference) {
        return false;
    }
    references.push(reference);
    let table = &program.type_reference_table;
    let symbol = match table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        }
        | TypeReferenceNode::DynamicTrait {
            symbol: base_symbol,
            arguments,
            ..
        } => {
            if *base_symbol == owner
                || table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| {
                        declaration_contains_family(
                            program,
                            owner,
                            *argument,
                            references,
                            declarations,
                        )
                    })
            {
                return true;
            }
            Some(*base_symbol)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            return declaration_contains_family(program, owner, *referee, references, declarations);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return declaration_contains_family(
                program,
                owner,
                *base_type,
                references,
                declarations,
            );
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            return declaration_contains_family(
                program,
                owner,
                *element_type,
                references,
                declarations,
            );
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => None,
    };
    let Some(symbol) = symbol else {
        return false;
    };
    if symbol == owner {
        return true;
    }
    if declarations.contains(&symbol) {
        return false;
    }
    declarations.push(symbol);
    let Some(data) = program.unique_data_definition_by_symbol(symbol) else {
        return false;
    };
    if let Some(quotient) = &data.quotient
        && declaration_contains_family(program, owner, quotient.carrier, references, declarations)
    {
        return true;
    }
    program
        .data_members(data)
        .iter()
        .any(|member| match member {
            DataMember::Field(field) => declaration_contains_family(
                program,
                owner,
                field.type_reference,
                references,
                declarations,
            ),
            DataMember::Variant(case) => program.data_payload_fields(case).iter().any(|field| {
                declaration_contains_family(
                    program,
                    owner,
                    field.type_reference,
                    references,
                    declarations,
                )
            }),
        })
}

#[derive(Clone)]
pub(super) struct SourceFamily {
    pub declaration: u32,
    pub symbol: SymbolHandle,
    pub parameters: Vec<TypeParameter>,
    pub common: Vec<DataField>,
    pub cases: Vec<SourceCase>,
    pub description: IndexedFamily,
    pub checked_inductions: Vec<induction::CheckedSourceInduction>,
}
#[derive(Clone)]
pub(super) struct SourceCase {
    pub symbol: SymbolHandle,
    pub payload: Vec<DataField>,
    pub children: Vec<DataField>,
    pub constraints: TermHandle,
    pub predicate_expressions: Vec<typed_trees::expression::ExpressionHandle>,
    pub child_indices: Vec<TermHandle>,
}

impl Elaborator<'_> {
    /// Seed source evidence demanded without any mathematical declaration or
    /// application. Each model still passes the same exact declaration/W checks.
    pub(super) fn demand_source_family_declarations(&mut self) -> Result<(), Vec<Diagnostic>> {
        for symbol in erased_family_demands(self.program) {
            self.source_family(symbol)?;
        }
        Ok(())
    }

    pub(super) fn indexed_application(
        &mut self,
        symbol: SymbolHandle,
        arguments: &[TypeReferenceHandle],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let family = self.source_family(symbol)?;
        if family.parameters.len() != arguments.len() {
            return Err(self.refuse("source indexed application changed its index arity".into()));
        }
        let mut term = self.constant(family.declaration);
        for (parameter, argument) in family.parameters.iter().zip(arguments) {
            let operand = self.index_operand(*argument, parameter, &[])?;
            term = self.arena.insert(Term::Apply {
                function: term,
                argument: operand,
            });
        }
        Ok(term)
    }

    pub(in super::super) fn source_family(
        &mut self,
        symbol: SymbolHandle,
    ) -> Result<SourceFamily, Vec<Diagnostic>> {
        if let Some(family) = self.indexed_families.get(&symbol) {
            return Ok(family.clone());
        }
        if self.indexed_family_stack.contains(&symbol) {
            return Err(
                self.refuse("nested source-family recursion has no checked encoding yet".into())
            );
        }
        let Some(data) = self
            .program
            .unique_data_definition_by_symbol(symbol)
            .cloned()
        else {
            return Err(self.refuse(
                "indexed carrier does not select an exact source data declaration".into(),
            ));
        };
        let parameters = self.program.data_type_parameters(&data).to_vec();
        if parameters.is_empty()
            || parameters.iter().any(|p| {
                !matches!(
                    p.kind,
                    TypeParameterKind::Type | TypeParameterKind::Value { .. }
                )
            })
        {
            return Err(self.refuse("source W descriptions currently require a Type/Value telescope; const/machine telescope denotation remains unsupported".into()));
        }
        if data.quotient.is_some()
            || data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            || !data.lifetime_parameters.is_empty()
        {
            return Err(self.refuse("quotient, opaque or region-bearing source indexed carriers require their own checked description".into()));
        }
        let saved_scope = std::mem::take(&mut self.scope);
        let saved_owner = self.source_owner;
        let saved_span = self.span;
        self.source_owner = symbol;
        self.span = self.program.symbols.symbol_source_span(symbol);
        self.indexed_family_stack.push(symbol);
        let result = self.build_source_family(&data, parameters);
        self.indexed_family_stack.pop();
        self.scope = saved_scope;
        self.source_owner = saved_owner;
        self.span = saved_span;
        let family = result?;
        // The exact description and alias are definitions in the original
        // scheme prefix. A malformed generated term never becomes a carrier.
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        proof_admission::check_signature(&mut self.arena, &self.declarations, &mut budget)
            .map_err(|error| {
                self.refuse(format!(
                    "generated source indexed-family description fails kernel checking: {error:?}"
                ))
            })?;
        self.indexed_families.insert(symbol, family.clone());
        Ok(family)
    }

    fn build_source_family(
        &mut self,
        data: &DataDefinition,
        parameters: Vec<TypeParameter>,
    ) -> Result<SourceFamily, Vec<Diagnostic>> {
        let mut common = Vec::new();
        let mut common_children = Vec::new();
        let mut variants = Vec::new();
        for member in self.program.data_members(data).to_vec() {
            match member {
                DataMember::Field(field) => {
                    if self
                        .direct_child(data.symbol, field.type_reference)?
                        .is_some()
                    {
                        common_children.push(field)
                    } else {
                        common.push(field)
                    }
                }
                DataMember::Variant(case) => variants.push(case),
            }
        }
        let index_fields = self
            .program
            .data_value_parameter_fields(data)
            .map(|(parameter, field)| (parameter.symbol, field.symbol))
            .collect::<Vec<_>>();
        if index_fields.len()
            != parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
                .count()
        {
            return Err(
                self.refuse("source indexed telescope lost its exact descriptor fields".into())
            );
        }
        let mut source_cases = Vec::new();
        let mut schemes = Vec::new();
        let rosters = if variants.is_empty() {
            vec![(SymbolHandle::invalid(), Vec::new(), data.where_facts)]
        } else {
            variants
                .iter()
                .map(|case| {
                    (
                        case.symbol,
                        self.program.data_payload_fields(case).to_vec(),
                        case.where_facts,
                    )
                })
                .collect()
        };
        for (case_symbol, fields, case_facts) in rosters {
            self.scope.clear();
            self.push_family_type_scope(&parameters);
            for field in &common {
                let domain = self.elaborate_type_reference(field.type_reference)?;
                self.push_scope(Some(field.symbol), field.name.clone(), domain);
            }
            let mut payload = Vec::new();
            let mut children = common_children.clone();
            let mut payload_types = Vec::new();
            for field in fields {
                if self
                    .direct_child(data.symbol, field.type_reference)?
                    .is_some()
                {
                    children.push(field);
                    continue;
                }
                let domain = self.closed_field_domain(&field)?;
                self.push_scope(Some(field.symbol), field.name.clone(), domain);
                payload_types.push((
                    field.symbol,
                    self.source_field_function(&field, &parameters)?,
                ));
                payload.push(field);
            }
            let mut predicates = self
                .program
                .proof_facts
                .span_or_empty(data.where_facts)
                .to_vec();
            if case_symbol.is_valid() {
                predicates.extend_from_slice(self.program.proof_facts.span_or_empty(case_facts));
            }
            let mut propositions = Vec::new();
            let mut predicate_expressions = Vec::new();
            for fact in predicates {
                let Some(expression) = fact.as_expression().copied() else {
                    return Err(self.refuse(
                        "source indexed constraint has no relevant proposition denotation yet"
                            .into(),
                    ));
                };
                let proposition = self.elaborate_proposition(expression)?.ok_or_else(|| {
                    self.refuse(
                        "source indexed constraint has no checked proposition denotation".into(),
                    )
                })?;
                predicate_expressions.push(expression);
                propositions.push(proposition);
            }
            let proposition = if let Some(mut body) = propositions.pop() {
                for preceding in propositions.into_iter().rev() {
                    let codomain = proof_admission::shift(&mut self.arena, body, 0, 1);
                    body = self.arena.insert(Term::Sigma {
                        domain: preceding,
                        codomain,
                    });
                }
                body
            } else {
                self.relevant_unit()
            };
            let constraints = self.close_family_telescope(proposition);
            let mut child_indices = Vec::new();
            for child in &children {
                let arguments = self
                    .direct_child(data.symbol, child.type_reference)?
                    .ok_or_else(|| {
                        self.refuse(
                            "source recursive field lost its exact family application".into(),
                        )
                    })?;
                if arguments.len() != parameters.len() {
                    return Err(
                        self.refuse("source recursive child changed the family index arity".into())
                    );
                }
                let mut indices = Vec::new();
                for (argument, parameter) in arguments.iter().zip(&parameters) {
                    indices.push(self.index_operand(*argument, parameter, &index_fields)?);
                }
                let index = self.index_tuple(indices)?;
                child_indices.push(self.close_family_telescope(index));
            }
            schemes.push(terms::Case {
                payload: payload_types,
                constraints,
                child_indices: child_indices.clone(),
            });
            source_cases.push(SourceCase {
                symbol: case_symbol,
                payload,
                children,
                constraints,
                predicate_expressions,
                child_indices,
            });
        }
        self.scope.clear();
        let mut common_pairs = common
            .iter()
            .map(|field| {
                self.source_field_function(field, &parameters)
                    .map(|ty| (field.symbol, ty))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let type_symbols = parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let type_sort = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        common_pairs.splice(0..0, type_symbols.iter().map(|symbol| (*symbol, type_sort)));
        let mut index_symbols = index_fields
            .iter()
            .map(|(_, field)| *field)
            .collect::<Vec<_>>();
        index_symbols.splice(0..0, type_symbols.iter().copied());
        let description = terms::description(
            &mut self.arena,
            &common_pairs,
            &index_symbols,
            &schemes,
            &type_symbols,
        )
        .map_err(|error| self.refuse(error.into()))?;
        let description =
            terms::intern_description(&mut self.arena, &mut self.declarations, description)
                .map_err(|error| self.refuse(error.into()))?;
        let mut domains = Vec::new();
        for parameter in &parameters {
            let domain = match parameter.kind {
                TypeParameterKind::Type => type_sort,
                TypeParameterKind::Value { type_reference } => {
                    self.elaborate_type_reference(type_reference)?
                }
                _ => unreachable!(),
            };
            domains.push(domain);
        }
        let indices = (0..parameters.len())
            .map(|ordinal| {
                self.arena
                    .insert(Term::Variable((parameters.len() - 1 - ordinal) as u32))
            })
            .collect();
        let index = self.index_tuple(indices)?;
        let mut body = description.indexed_w(&mut self.arena, index);
        let mut ty = self.arena.insert(Term::Sort(Sort::Type(
            description.levels[0]
                .clone()
                .maximum(description.levels[1].clone())
                .maximum(description.levels[2].clone()),
        )));
        for domain in domains.into_iter().rev() {
            body = self.arena.insert(Term::Lambda { domain, body });
            ty = self.arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        let declaration = self.declarations.len() as u32;
        self.declarations.push(Declaration {
            level_arity: 0,
            ty,
            body: Some(body),
        });
        Ok(SourceFamily {
            declaration,
            symbol: data.symbol,
            parameters,
            common,
            cases: source_cases,
            description,
            checked_inductions: Vec::new(),
        })
    }

    fn direct_child(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Result<Option<Vec<TypeReferenceHandle>>, Vec<Diagnostic>> {
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } if *base_symbol == owner => {
                if !lifetime_arguments.is_empty() {
                    return Err(self.refuse(
                        "recursive indexed fields cannot discard region arguments".into(),
                    ));
                }
                Ok(Some(
                    self.program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec(),
                ))
            }
            _ => {
                if self.contains_family(owner, reference, &mut Vec::new(), &mut Vec::new()) {
                    return Err(self.refuse(
                        "nested or mutual source-family recursion has no checked encoding yet"
                            .into(),
                    ));
                }
                Ok(None)
            }
        }
    }

    /// Inspect exact declaration edges before opaque ordinary carrier
    /// elaboration. A nominal wrapper must not hide a recursive occurrence
    /// from the source-to-W correspondence. This is an unsupported-encoding
    /// check; it does not claim that every nested occurrence is negative.
    fn contains_family(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
        references: &mut Vec<TypeReferenceHandle>,
        declarations: &mut Vec<SymbolHandle>,
    ) -> bool {
        declaration_contains_family(self.program, owner, reference, references, declarations)
    }
    pub(super) fn push_family_type_scope(&mut self, parameters: &[TypeParameter]) {
        for parameter in parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        {
            let domain = self
                .arena
                .insert(Term::Sort(Sort::Type(Level::Constant(0))));
            self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
        }
    }

    /// A payload carrier is a closed function of the exact source Type prefix.
    /// The shared W builder applies it under keyed dependent label binders.
    fn source_field_function(
        &mut self,
        field: &DataField,
        parameters: &[TypeParameter],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let saved = std::mem::take(&mut self.scope);
        self.push_family_type_scope(parameters);
        let result = self
            .kernel_field_domain(field)
            .map(|body| self.close_family_telescope(body));
        self.scope = saved;
        result
    }

    fn closed_field_domain(&mut self, field: &DataField) -> Result<TermHandle, Vec<Diagnostic>> {
        let domain = self.elaborate_type_reference(field.type_reference)?;
        let context = self.scope.iter().fold(
            Context::empty()
                .with_signature(Signature::from_declarations(self.declarations.clone())),
            |context, entry| context.extend(entry.domain),
        );
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        proof_admission::infer_sort(&mut self.arena,&context,domain,&mut budget).map_err(|_|self.refuse(format!("indexed payload `{}` has an open dependent carrier whose encoding is unsupported",field.name)))?;
        Ok(domain)
    }
    /// The W representation may use the shared primitive calculus carrier.
    /// Only aliases created by the fixed source-carrier producer qualify;
    /// arbitrary transparent source declarations remain untouched. Exact
    /// field references, validation scopes and family application domains
    /// retain their declared source carriers for correspondence checking.
    fn kernel_field_domain(&mut self, field: &DataField) -> Result<TermHandle, Vec<Diagnostic>> {
        let domain = self.closed_field_domain(field)?;
        Ok(self.kernel_primitive_domain(domain))
    }
    fn kernel_primitive_domain(&mut self, domain: TermHandle) -> TermHandle {
        let Term::Constant {
            declaration,
            levels,
        } = self.arena.get(domain)
        else {
            return domain;
        };
        if !levels.is_empty() {
            return domain;
        }
        let carrier = self
            .source_aliases
            .iter()
            .find(|alias| alias.declaration == declaration)
            .map(|alias| alias.carrier);
        carrier
            .map(|carrier| self.carrier_term(carrier))
            .unwrap_or(domain)
    }
    fn close_family_telescope(&mut self, mut body: TermHandle) -> TermHandle {
        let domains = self
            .scope
            .iter()
            .map(|entry| entry.domain)
            .collect::<Vec<_>>();
        for domain in domains.into_iter().rev() {
            // Source expressions and recursive indices were checked under
            // the exact declared scope. Compact only the generated lambda
            // annotation, preserving the complete authored body unchanged.
            let domain = self.kernel_primitive_domain(domain);
            body = self.arena.insert(Term::Lambda { domain, body });
        }
        body
    }
    fn relevant_unit(&mut self) -> TermHandle {
        let empty = self.arena.insert(Term::Empty);
        let unit = self.arena.insert(Term::Pi {
            domain: empty,
            codomain: empty,
        });
        self.arena.insert(Term::Box { ty: unit })
    }
    fn index_tuple(&mut self, mut indices: Vec<TermHandle>) -> Result<TermHandle, Vec<Diagnostic>> {
        let Some(mut tuple) = indices.pop() else {
            return Err(self.refuse("indexed family has no index tuple".into()));
        };
        for index in indices.into_iter().rev() {
            tuple = self.arena.insert(Term::Pair {
                first: index,
                second: tuple,
            });
        }
        Ok(tuple)
    }
    pub(in super::super) fn index_operand(
        &mut self,
        reference: TypeReferenceHandle,
        parameter: &TypeParameter,
        aliases: &[(SymbolHandle, SymbolHandle)],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        if matches!(parameter.kind, TypeParameterKind::Type) {
            return self.elaborate_type_reference(reference);
        }
        let TypeParameterKind::Value {
            type_reference: carrier,
        } = parameter.kind
        else {
            unreachable!("family producer retains only Value indices")
        };
        let node = self
            .program
            .type_reference_table
            .type_reference(reference)
            .clone();
        match node {
            TypeReferenceNode::Named { symbol, name } if symbol.is_valid() => {
                let canonical = aliases
                    .iter()
                    .find(|(source, _)| *source == symbol)
                    .map(|(_, field)| *field)
                    .unwrap_or(symbol);
                let term = self.named_term(canonical, &name)?;
                let expected = self.elaborate_type_reference(carrier)?;
                let signature = Signature::from_declarations(self.declarations.clone());
                let context = self.scope.iter().fold(
                    Context::empty().with_signature(signature.clone()),
                    |context, entry| context.extend(entry.domain),
                );
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                let actual =
                    proof_admission::infer_type(&mut self.arena, &context, term, &mut budget)
                        .map_err(|error| {
                            self.refuse(format!("source index has no checked domain: {error:?}"))
                        })?;
                if !self.source_domains_correspond(
                    &signature,
                    &context,
                    expected,
                    actual,
                    None,
                    &mut budget,
                )? {
                    return Err(self.refuse(format!(
                        "source index `{name}` has a different declared carrier"
                    )));
                }
                Ok(term)
            }
            TypeReferenceNode::Named { symbol: _, name } => {
                validation::validate_closed_const_argument(
                    self.program,
                    "source indexed family",
                    parameter,
                    reference,
                    &[],
                    &[],
                )?;
                let value = BigInt::from_decimal_str(name.as_str())
                    .or_else(|| {
                        match CanonicalConstValue::from_atom(name.as_str())?.decode_encoding()? {
                            DecodedCanonicalConstValue::Integer { value, .. } => {
                                Some(BigInt::from_i128(value))
                            }
                            _ => None,
                        }
                    })
                    .ok_or_else(|| {
                        self.refuse(format!(
                            "source index `{name}` has no checked scalar denotation"
                        ))
                    })?;
                if self.scalar_carrier_of_type_reference(carrier) != Some(ScalarCarrier::Integer) {
                    return Err(self.refuse(
                        "integer index does not inhabit the declared source carrier".into(),
                    ));
                }
                let integer = self.integer_carrier();
                self.numeric_literal(integer, value)
            }
            TypeReferenceNode::ConstExpression(expression) => {
                if !validation::checked_argument_matches_type_reference(
                    self.program,
                    expression,
                    carrier,
                ) {
                    return Err(self.refuse(
                        "source index expression does not inhabit its declared carrier".into(),
                    ));
                }
                self.elaborate_expression(expression)
            }
            _ => {
                Err(self
                    .refuse("source index operand is not a bound value or closed scalar".into()))
            }
        }
    }
}
