/*
Keep carrier conversion separate from changes to a value's qualification.
Domain-only casts preserve the same value and carrier, with shared accounting
for executable expressions and contract terms. Adding a domain owes its issuer
and predicate checks. Removal keeps ownership and cannot erase access, cleanup
or borrowed restoration obligations.

Carrier casts start with an unqualified operand and perform an exact scalar
conversion before adding a policy. Float widening uses the authenticated Core
conversion whose entire input format is exactly representable in its result.
Collection recasts retain their written reference type and use collection-view
formation. Other storage recasts require an explicit reference formation.
*/

use crate::qualifications::explain_policy_refusal;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use crate::source_state::types::TypeReferenceNode;
use language_core::ReferenceAccess;
use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};

use super::{Program, Slot};
use crate::body::{
    DomainInstance, DomainSet, Expression, ExpressionKind, Policy, Qualification, ValueType,
};
use crate::{SourceOrigin, Unserved, refuse, reject};

#[derive(Clone, Copy)]
pub(super) enum CastContext<'a> {
    Body(&'a [Slot]),
    Clause,
}

pub(super) struct DomainCast {
    pub(super) value: ExpressionHandle,
    pub(super) input: ValueType,
    pub(super) result: ValueType,
    added: DomainSet,
    tests: Vec<(Expression, DomainInstance)>,
    pub(super) removed: Vec<(DomainInstance, SourceOrigin)>,
}

impl Program<'_> {
    pub(super) fn cast(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        cast: &TableCastExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        if let Some(entry) = cast.form.entry() {
            return self.domain_entry(id, slots, handle, cast, entry, source);
        }
        if let Some(access) = cast.form.recast_access() {
            if access != ReferenceAccess::Shared {
                return unserved(
                    "exclusive storage view without establishment and handback evidence",
                );
            }
            let reference = self
                .typed
                .expression_table
                .constructed_value_type_reference(handle);
            if !self
                .typed
                .type_reference_table
                .contains_type_reference(reference)
            {
                return unserved("storage view outside reference formation");
            }
            if !matches!(
                self.typed.type_reference_table.type_reference(reference),
                TypeReferenceNode::Reference {
                    referee,
                    access: retained,
                    ..
                } if *referee == cast.target_type && *retained == access
            ) {
                return unserved("storage view differs from its retained reference type");
            }
            if let Some(view) = self.view_type(id, reference)? {
                return self.collection_recast(id, slots, cast, reference, view, source);
            }
            let expected = self.value_type(id, reference)?;
            return self.borrow_value(id, slots, handle, expected, source);
        }
        if cast.semantic_domain_symbol.is_valid() || !cast.semantic_domain.is_empty() {
            return unserved("cast naming a domain");
        }
        // A represented scalar conversion carries its target and policy separately.
        use numerics::arithmetic::ArithmeticDomain;
        let named = Policy::of_domain(cast.domain);
        let target_type = self.value_type(id, cast.target_type)?;
        if named.is_none()
            && let Some(quotient) = self.executable_quotient_of(target_type).cloned()
        {
            if cast.result_type.is_valid() && self.value_type(id, cast.result_type)? != target_type
            {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("cast result type"),
                ));
            }
            let operand = self.expression(id, slots, cast.value, Some(quotient.carrier))?;
            if operand.result != Some(quotient.carrier) {
                return unserved("quotient construction operand does not match its carrier");
            }
            return Ok(Expression {
                call_context: None,
                kind: ExpressionKind::Record {
                    fields: vec![(0, operand)],
                    erased: Vec::new(),
                },
                result: Some(target_type),
                source,
            });
        }
        let target = match (target_type, named) {
            (
                ValueType::Scalar(carrier @ ScalarType::Integer(_), Qualification::NONE),
                Some(policy),
            ) => ValueType::Scalar(carrier, Qualification::of_policy(policy)),
            (target @ ValueType::Scalar(..), None) => target,
            (ValueType::Scalar(..), Some(_)) => {
                return unserved("cast that names a policy for a type that cannot carry one");
            }
            (ValueType::Record(_, _, _) | ValueType::Rigid(_), _) => {
                return unserved("cast to a type that is not a scalar");
            }
        };
        if cast.result_type.is_valid() && self.value_type(id, cast.result_type)? != target {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("cast result type"),
            ));
        }
        // An explicit carrier supplies the landing type for an anonymous
        // numeric operand, including the carrier step before a named domain.
        let landing = target
            .scalar()
            .filter(|_| self.wholly_anonymous(cast.value));
        let operand = match landing {
            // `as` to a computational integer type lands the complete
            // anonymous operand, fractional intermediates included.
            Some(integer @ ScalarType::Integer(_))
                if !matches!(
                    self.typed.expression_table.expression(cast.value),
                    crate::source_state::expression::ExpressionNode::Integer(_)
                ) =>
            {
                let evaluated = self.anonymous_value(
                    id,
                    cast.value,
                    Some(integer),
                    super::types::AnonymousLanding::ExplicitAs,
                )?;
                let owner = self.symbols.get(id).copied().unwrap_or_default();
                self.complete_anonymous_operators(id, owner, &evaluated.operators)?;
                Expression {
                    call_context: None,
                    kind: ExpressionKind::Integer(evaluated.value),
                    result: Some(ValueType::plain(ScalarType::Integer(evaluated.integer))),
                    source: SourceOrigin::of(self.typed.expression_table.source_span(cast.value)),
                }
            }
            _ => self.expression(id, slots, cast.value, landing.map(ValueType::plain))?,
        };
        let source_type = match operand.result {
            Some(source_type @ ValueType::Scalar(..)) => source_type,
            Some(ValueType::Record(_, _, _) | ValueType::Rigid(_)) | None => {
                return unserved("cast of a non-scalar");
            }
        };
        // domains.md, "Carrier casts": "The operand of a carrier cast must
        // carry no domain", and of a qualified value moved to another
        // carrier, "Rejected: each of these changes the carrier of a
        // qualified value". A declared domain is added and removed by an
        // entry (`domain_entry`), never by this cast.
        if !source_type.qualification().domains.is_empty()
            || (source_type.policy().is_some() && source_type.scalar() != target.scalar())
        {
            return Err(reject(
                id,
                source,
                format!(
                    "this cast names a carrier and its operand's type carries {}: the operand \
                     of a carrier cast carries no domain, so each domain is removed by name \
                     first (`value as -D as T`)",
                    self.domains_named(source_type.qualification()),
                ),
            ));
        }
        if !target.qualification().domains.is_empty() {
            return unserved("carrier cast into a type that carries a declared domain");
        }
        // The carrier first: the same carrier converts nothing, and any
        // other is the one exact conversion, whatever policy the cast then
        // adds ("`plain as i64 +Km` abbreviates `(plain as i64) as +Km`").
        // No policy makes it a truncation or a clamp: "`as` never calls
        // user code, rounds, saturates, traps".
        let converted = if matches!(
            (source_type.scalar(), target.scalar()),
            (Some(from), Some(to)) if from == to
        ) {
            operand
        } else if let (Some(ScalarType::IeeeFloat(from)), Some(ScalarType::IeeeFloat(to))) =
            (source_type.scalar(), target.scalar())
        {
            use numerics::intrinsics::NumericIntrinsic;
            use numerics::intrinsics::NumericType;
            let numeric = |format| match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => NumericType::F32,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => NumericType::F64,
            };
            let operand = if from.can_exactly_widen_to(to) {
                operand
            } else {
                // The obligation names only this source, before the rounded
                // conversion's result exists. A live guarantee must establish
                // the same-format roundtrip; the goal cannot establish itself.
                let source_term = semantic_vocabulary::ScalarTerm::value(
                    semantic_vocabulary::ValueId::new(1).expect("fixed predicate formal"),
                    ScalarType::IeeeFloat(from),
                );
                let clause = semantic_vocabulary::ClauseIdentity::float_conversion_roundtrip(
                    source_term,
                    to,
                )
                .map_err(|detail| refuse(id, Construct::Machine, Unserved::Expression(detail)))?;
                Expression {
                    result: operand.result,
                    kind: ExpressionKind::Establish {
                        operand: Box::new(operand),
                        clauses: vec![semantic_vocabulary::Proposition::DeclarationIdentity(clause)],
                        stated: terminal_check_tables::program::EstablishedCondition::ExactRepresentation(
                            format!(
                                "the float value is exactly representable in {}",
                                match to {
                                    semantic_vocabulary::IeeeFloatFormat::Binary32 => "f32",
                                    semantic_vocabulary::IeeeFloatFormat::Binary64 => "f64",
                                }
                            ),
                        ),
                    },
                    call_context: None,
                    source,
                }
            };
            self.exact_numeric_conversion(
                id,
                NumericIntrinsic::Conversion {
                    source: numeric(from),
                    target: numeric(to),
                    domain: ArithmeticDomain::Exact,
                },
                operand,
                source,
            )?
        } else if let (Some(ScalarType::IeeeFloat(from)), Some(ScalarType::Integer(to))) =
            (source_type.scalar(), target.scalar())
        {
            use numerics::intrinsics::NumericIntrinsic;
            use numerics::intrinsics::NumericType;
            let carrier = semantic_vocabulary::fixed_integer_type(to)
                .map_err(|detail| refuse(id, Construct::Machine, Unserved::Expression(detail)))?;
            let source_term = semantic_vocabulary::ScalarTerm::value(
                semantic_vocabulary::ValueId::new(1).expect("fixed predicate formal"),
                ScalarType::IeeeFloat(from),
            );
            let operand = Expression {
                result: operand.result,
                kind: ExpressionKind::Establish {
                    operand: Box::new(operand),
                    clauses: vec![semantic_vocabulary::Proposition::IeeeFloatIntegerExact {
                        source: source_term,
                        target: to,
                    }],
                    stated:
                        terminal_check_tables::program::EstablishedCondition::ExactRepresentation(
                            format!("the float value is exactly representable in {carrier}"),
                        ),
                },
                call_context: None,
                source,
            };
            self.exact_numeric_conversion(
                id,
                NumericIntrinsic::Conversion {
                    source: match from {
                        semantic_vocabulary::IeeeFloatFormat::Binary32 => NumericType::F32,
                        semantic_vocabulary::IeeeFloatFormat::Binary64 => NumericType::F64,
                    },
                    target: NumericType::Integer(carrier),
                    domain: ArithmeticDomain::Exact,
                },
                operand,
                source,
            )?
        } else if let (Some(ScalarType::Integer(from)), Some(ScalarType::IeeeFloat(to))) =
            (source_type.scalar(), target.scalar())
        {
            use numerics::intrinsics::NumericIntrinsic;
            use numerics::intrinsics::NumericType;
            let carrier = semantic_vocabulary::fixed_integer_type(from)
                .map_err(|detail| refuse(id, Construct::Machine, Unserved::Expression(detail)))?;
            let source_term = semantic_vocabulary::ScalarTerm::value(
                semantic_vocabulary::ValueId::new(1).expect("fixed predicate formal"),
                ScalarType::Integer(from),
            );
            let operand = Expression {
                result: operand.result,
                kind: ExpressionKind::Establish {
                    operand: Box::new(operand),
                    clauses: vec![semantic_vocabulary::Proposition::IntegerIeeeFloatExact {
                        source: source_term,
                        target: to,
                    }],
                    stated:
                        terminal_check_tables::program::EstablishedCondition::ExactRepresentation(
                            format!(
                                "the integer value is exactly representable in {}",
                                match to {
                                    semantic_vocabulary::IeeeFloatFormat::Binary32 => "f32",
                                    semantic_vocabulary::IeeeFloatFormat::Binary64 => "f64",
                                }
                            ),
                        ),
                },
                call_context: None,
                source,
            };
            self.exact_numeric_conversion(
                id,
                NumericIntrinsic::Conversion {
                    source: NumericType::Integer(carrier),
                    target: match to {
                        semantic_vocabulary::IeeeFloatFormat::Binary32 => NumericType::F32,
                        semantic_vocabulary::IeeeFloatFormat::Binary64 => NumericType::F64,
                    },
                    domain: ArithmeticDomain::Exact,
                },
                operand,
                source,
            )?
        } else {
            let (Some(ScalarType::Integer(from)), Some(carrier @ ScalarType::Integer(to))) =
                (source_type.scalar(), target.scalar())
            else {
                return unserved("cast whose exact carrier conversion is not supported");
            };
            // counts_and_addresses.md: "neither arbitrary value-to-address
            // conversions nor the inverse ... follow from these queries".
            if from.is_address() || to.is_address() {
                return unserved("cast to or from an address");
            }
            if !from.can_widen_to(to) && !from.can_exact_cast_to(to) {
                return unserved("cast with no exact conversion");
            }
            Expression {
                call_context: None,
                kind: ExpressionKind::Cast(Box::new(operand)),
                result: Some(ValueType::Scalar(carrier, Qualification::NONE)),
                source,
            }
        };
        // Then the policy. Adding or removing one keeps the value:
        // numeric_values.md, "Adding and removing a policy domain ...
        // change future operations, not existing payloads".
        if converted.result == Some(target) {
            return Ok(converted);
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Requalify(Box::new(converted)),
            result: Some(target),
            source,
        })
    }

    /// Cast formation establishes exactness before this call. Publication and
    /// lowering retain the canonical operation's complete equation, source
    /// identity and target controls, just as for an ordinary numeric call.
    fn exact_numeric_conversion(
        &mut self,
        id: MachineId,
        meaning: numerics::intrinsics::NumericIntrinsic,
        operand: Expression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Expression(detail));
        let declaration = terminal_semantics::numeric_primitives::declaration_path(meaning)
            .map_err(unavailable)?;
        let target = self.typed.symbols
            .find_top_level_by_name_and_kinds_from_source_matching(
                &declaration,
                &[symbols::SymbolKind::Machine, symbols::SymbolKind::Operator],
                source.span().unwrap_or_default(),
                |symbol| {
                    self.typed.symbols.sealed_intrinsic(symbol)
                        == Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement)
                        && self.typed.numeric_intrinsic_execution(symbol) == Some(meaning)
                },
            ).ok_or_else(|| unavailable("exact carrier conversion has no canonical declaration"))?;
        let boundary = self.boundary_declaration(id, target)?;
        let signature = self
            .boundaries
            .signature(boundary)
            .ok_or_else(|| unavailable("exact carrier conversion lost its signature"))?;
        if signature.parameters.len() != 1
            || operand.result != signature.parameters.first().copied()
        {
            return Err(unavailable(
                "exact carrier conversion changes its operand format",
            ));
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Call {
                callee: crate::body::CallTarget::Boundary { boundary },
                required_contract: None,
                static_reach_binding: None,
                static_reach_arguments: Vec::new(),
                proof_arguments: Vec::new(),
                arguments: vec![operand],
                designated: None,
            },
            result: signature.result,
            source,
        })
    }
}

impl Program<'_> {
    /// Interpret the source value once. All entries of this cast share one
    /// result and one proof site, including entries written in reverse order.
    fn domain_entry(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let elision = std::mem::replace(&mut self.elision, super::regions::Elision::Variables);
        let run = self.read_domain_cast(id, CastContext::Body(slots), handle, cast, entry);
        self.elision = elision;
        let DomainCast {
            value,
            input,
            mut result,
            added,
            tests,
            removed,
        } = run?;
        let landing =
            if self.reference_referent(id, result)?.is_some() || self.is_borrowed_view(result) {
                Some(input)
            } else if self.wholly_anonymous(value) {
                result.scalar().map(ValueType::plain)
            } else if self.byte_literal_operand(value) || self.array_literal_operand(value) {
                result.qualified(Qualification::NONE)
            } else {
                None
            };
        let operand = self
            .expression(id, slots, value, landing)?
            .with_membership_tests(tests.into_iter().rev());
        let unavailable = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let actual = operand
            .result
            .ok_or_else(|| unavailable("domain cast of a Unit value"))?;
        if actual.erased() != input.erased() {
            return Err(unavailable(
                "domain cast does not account for the operand's qualifications",
            ));
        }
        // A cast preserves the operand's regions, including a literal's
        // lifetime selected at use. Only its displayed domains change.
        if let (ValueType::Record(row, qualification, _), ValueType::Record(_, _, regions)) =
            (result, actual)
        {
            result = ValueType::Record(row, qualification, regions);
        }
        let structural_replacement =
            matches!(actual, ValueType::Record(..)) && !removed.is_empty() && !added.is_empty();
        let mut retained = actual.qualification();
        for (domain, origin) in removed.into_iter().rev() {
            self.domain_removal(id, domain, actual, origin)?;
            retained.domains = retained
                .domains
                .without(domain)
                .ok_or_else(|| unavailable("domain cast removes an absent membership"))?;
        }
        if actual == result && added.is_empty() {
            return Ok(operand);
        }
        // Structural changes strictly erase or strictly add on one edge.
        // The common qualifications survive both steps of a replacement.
        let operand = if structural_replacement {
            Expression {
                call_context: None,
                kind: ExpressionKind::Requalify(Box::new(operand)),
                result: actual.qualified(retained),
                source,
            }
        } else {
            operand
        };
        self.establish_domains(id, operand, result, added, source)
    }

    /// Recover the input from the signed entries and verify typing's
    /// accounting coordinates. Only the complete cast presents a new type.
    pub(super) fn read_domain_cast(
        &mut self,
        id: MachineId,
        context: CastContext<'_>,
        mut handle: ExpressionHandle,
        first_read: &TableCastExpression,
        mut entry: language_core::cast_form::CastEntry,
    ) -> Checked<DomainCast> {
        use language_core::cast_form::CastEntryKind;
        let unavailable = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let mut site = *first_read;
        if !site.result_type.is_valid() {
            return Err(unavailable("domain cast typing did not account for"));
        }
        let result = match context {
            CastContext::Body(_) => self.value_type(id, site.result_type)?,
            CastContext::Clause => self.qualified_scalar_type(id, site.result_type)?,
        };
        let mut after = result;
        let mut removed = Vec::new();
        let mut added = DomainSet::EMPTY;
        let mut tests = Vec::new();
        loop {
            if matches!(context, CastContext::Clause) && entry.kind == CastEntryKind::Add {
                // A clause cannot assume its own assertion to establish a domain.
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("domain establishment in a clause term"),
                ));
            }
            let named = self.entry_domains(id, &site)?;
            let before = self.entry_input(id, &site, entry, after, named)?;
            self.domain_entry_change(id, before, after, &site, entry, named)?;
            let input = before.qualification().domains;
            let output = after.qualification().domains;
            match entry.kind {
                CastEntryKind::Add => {
                    for gained in output.members().filter(|domain| !input.contains(*domain)) {
                        added = added
                            .with(gained)
                            .ok_or_else(|| unavailable("domain cast repeats an addition"))?;
                    }
                }
                CastEntryKind::Remove => {
                    let at = SourceOrigin::of(self.typed.expression_table.source_span(handle));
                    removed.extend(named.members().map(|domain| (domain, at)));
                }
                CastEntryKind::Keep => {}
            }
            if entry.first {
                return Ok(DomainCast {
                    value: site.value,
                    input: before,
                    result,
                    added,
                    tests,
                    removed,
                });
            }
            // Typing allocates nested operands before their consumers.
            if site.value.arena_index() >= handle.arena_index() {
                return Err(unavailable("domain cast lost its earlier entry"));
            }
            handle = site.value;
            let ExpressionNode::Cast(previous) = self.node(id, handle)? else {
                return Err(unavailable(
                    "domain cast lost an entry of its simultaneous run",
                ));
            };
            if let CastContext::Body(slots) = context {
                // The run is read from its outermost entry back to its
                // operand. Restore source order when wrapping that operand.
                for test in self
                    .typed
                    .expression_table
                    .executable_domain_tests(handle)
                    .iter()
                    .rev()
                {
                    if let Some(question) = self.require_membership_carrier(id, slots, *test)? {
                        tests.push(question);
                    }
                }
            }
            site = *previous;
            entry = site
                .form
                .entry()
                .ok_or_else(|| unavailable("domain cast lost its first entry"))?;
            if !site.result_type.is_valid()
                || self.cast_coordinate_type(id, site.result_type)?.erased() != before.erased()
            {
                return Err(unavailable(
                    "domain cast entries disagree on their qualifications",
                ));
            }
            after = before;
        }
    }

    /// The selected declaration is retained for provenance. A transparent
    /// alias names its leaves, each with its own establishment and custody.
    fn entry_domains(&self, id: MachineId, cast: &TableCastExpression) -> Checked<DomainSet> {
        if Policy::of_domain(cast.domain).is_some() {
            return Ok(DomainSet::EMPTY);
        }
        let selected = crate::body::DomainInstance {
            declaration: cast.semantic_domain_symbol,
            identity: cast.semantic_domain_id,
        };
        let definition = self.domain_theory(id, selected)?;
        if definition.alias.is_none() {
            return Ok(DomainSet::EMPTY.with(selected).expect("one domain fits"));
        }
        let invalid = |reason| refuse(id, Construct::Machine, Unserved::Type(reason));
        let mut named = DomainSet::EMPTY;
        for (member, identity) in
            crate::source_state::domain::expanded_alias_members(self.typed, selected.declaration)
                .map_err(invalid)?
        {
            named = named
                .with(crate::body::DomainInstance {
                    declaration: member.symbol,
                    identity,
                })
                .ok_or_else(|| invalid("more than four declared domains"))?;
        }
        Ok(named)
    }

    /// Recover the input of this signed entry from its retained output.
    fn entry_input(
        &mut self,
        id: MachineId,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        result: ValueType,
        named: DomainSet,
    ) -> Checked<ValueType> {
        use language_core::cast_form::CastEntryKind;
        let unavailable = || {
            refuse(
                id,
                Construct::Machine,
                Unserved::Typing("domain cast result does not account for its input domains"),
            )
        };
        if entry.kind == CastEntryKind::Add {
            let written = self.cast_coordinate_type(id, cast.target_type)?;
            if written.erased().qualified(Qualification::NONE)
                != result.erased().qualified(Qualification::NONE)
            {
                return Err(unavailable());
            }
            return result
                .qualified(written.qualification())
                .ok_or_else(unavailable);
        }
        let mut before = result.qualification();
        if let Some(policy) = Policy::of_domain(cast.domain) {
            before.policy = match (entry.kind, before.policy) {
                (CastEntryKind::Remove, None) => Some(policy),
                (CastEntryKind::Keep, Some(after)) if after == policy => Some(after),
                _ => return Err(unavailable()),
            };
        } else if entry.kind == CastEntryKind::Remove {
            for domain in named.members() {
                before.domains = before.domains.with(domain).ok_or_else(unavailable)?;
            }
        }
        result.qualified(before).ok_or_else(unavailable)
    }

    /// Add a domain after checking its predicates. Canonical checking decides
    /// whether the value already proves membership or its author may issue it.
    /// Fresh zero construction separately requires issuance authority.
    pub(super) fn add_domain(
        &mut self,
        id: MachineId,
        operand: Expression,
        result: ValueType,
        domain: crate::body::DomainInstance,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let added = DomainSet::EMPTY.with(domain).expect("one domain fits");
        self.establish_domains(id, operand, result, added, source)
    }

    fn establish_domains(
        &mut self,
        id: MachineId,
        operand: Expression,
        result: ValueType,
        added: DomainSet,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let mut predicates = false;
        for domain in added.members() {
            predicates |= super::authority::has_predicates(self.domain_theory(id, domain)?);
        }
        let qualification = Qualification {
            policy: None,
            domains: added,
        };
        let (clauses, stated) = match result {
            _ if !predicates => (Vec::new(), String::new()),
            ValueType::Scalar(carrier, _) => self.cast_predicates(id, qualification, carrier)?,
            ValueType::Record(..) => self.record_cast_predicates(id, qualification, result)?,
            ValueType::Rigid(_) => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Expression(
                        "cast that adds a domain with predicates to a value of a type parameter",
                    ),
                ));
            }
        };
        if !clauses.is_empty() {
            return Ok(Expression {
                call_context: None,
                kind: ExpressionKind::Establish {
                    operand: Box::new(operand),
                    clauses,
                    stated: terminal_check_tables::program::EstablishedCondition::DomainMembership(
                        stated,
                    ),
                },
                result: Some(result),
                source,
            });
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Requalify(Box::new(operand)),
            result: Some(result),
            source,
        })
    }

    /// Check the same domain accounting in executable and logical casts.
    /// Changing qualification keeps the complete carrier, including regions
    /// and ownership. An entry changes exactly its selected domain or policy.
    fn domain_entry_change(
        &self,
        id: MachineId,
        operand: ValueType,
        result: ValueType,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        named: DomainSet,
    ) -> Checked<()> {
        use language_core::cast_form::CastEntryKind;
        let mistyped = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
        if operand.qualified(Qualification::NONE) != result.qualified(Qualification::NONE)
            || operand.qualified(Qualification::NONE).is_none()
        {
            return Err(mistyped("domain cast entry that changes the carrier"));
        }
        let (before, after) = (operand.qualification(), result.qualification());
        if entry.kind == CastEntryKind::Keep {
            if before != after {
                return Err(mistyped("kept domain that changes the type"));
            }
            if !named.is_subset_of(&before.domains) {
                return Err(mistyped("kept domain that the operand does not carry"));
            }
            return Ok(());
        }
        let (wider, narrower) = match entry.kind {
            CastEntryKind::Remove => (before, after),
            CastEntryKind::Add => (after, before),
            CastEntryKind::Keep => return Err(mistyped("kept domain without an unchanged type")),
        };
        let invalid =
            || mistyped("domain cast entry that does not change exactly the domain it names");
        if let Some(policy) = Policy::of_domain(cast.domain) {
            return if wider.policy == Some(policy)
                && narrower.policy.is_none()
                && wider.domains == narrower.domains
            {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        if !narrower.domains.is_subset_of(&wider.domains)
            || narrower.policy != wider.policy
            || !named.is_subset_of(&wider.domains)
            || named
                .members()
                .any(|domain| narrower.domains.contains(domain))
        {
            return Err(invalid());
        }
        let mut allowed = named;
        if entry.kind == CastEntryKind::Add {
            allowed = DomainSet::EMPTY;
            for domain in named.members() {
                allowed = self.with_refinement(id, allowed, domain)?;
            }
        }
        if wider
            .domains
            .members()
            .any(|d| !narrower.domains.contains(d) && !allowed.contains(d))
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// Issuance restricts adding a domain, not forgetting its displayed
    /// qualification. Removal keeps value facts and the carrier's custody;
    /// access, cleanup and borrowed restoration obligations cannot disappear.
    pub(super) fn domain_removal(
        &self,
        id: MachineId,
        domain: crate::body::DomainInstance,
        operand: ValueType,
        source: SourceOrigin,
    ) -> Checked<()> {
        let unavailable = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let definition = self.domain_theory(id, domain)?;
        if definition.requires_access_retention() {
            return Err(unavailable(
                "removal of a domain governed by an access policy",
            ));
        }
        if definition.cleanup_selection.is_some() {
            return Err(unavailable("removal of a domain with cleanup obligations"));
        }
        // An exclusive view owes this domain back when its loan ends. Its
        // lender must therefore have the same authority as an explicit +D.
        if let ValueType::Record(row, _, _) = operand
            && matches!(
                self.data_type(id, row)?.shape,
                crate::StorageShape::Reference {
                    access: terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    ..
                } | crate::StorageShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                    } | terminal_psi::ByteSequenceCarrier::BoundedView {
                        access: Some(
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        ),
                        ..
                    }
                ) | crate::StorageShape::ElementView {
                    access: Some(
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    ),
                    ..
                }
            )
        {
            self.issue(id, domain, source)?;
        }
        Ok(())
    }

    /// Fresh values and mutable borrow restoration need the author's permission
    /// to establish a domain. Forwarding existing membership is judged separately
    /// on the lowered value, after calls and storage writes are known.
    pub(super) fn issue(
        &self,
        id: MachineId,
        domain: crate::body::DomainInstance,
        source: SourceOrigin,
    ) -> Checked<()> {
        use crate::source_state::domain::DomainIssuance;
        use source_declarations::DomainEstablishmentRoute;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let definition = self.domain_theory(id, domain)?;
        match definition.issuance() {
            DomainIssuance::Unrestricted => {}
            DomainIssuance::Only(routes) => {
                let machine =
                    *terminal_check_tables::row(&self.symbols, id, id, Construct::Machine)?;
                // authority.md, "Requirement and exact-machine routes":
                //
                // | Trait requirement | An exact invocation through that
                //   requirement and a valid selected conformance. This
                //   includes a call by name to a machine that declares that
                //   edge ... |
                // | Plain machine declaration | That exact machine's direct
                //   invocation. |
                //
                // A machine that declares the edge (`machine issue(..)
                // satisfies Issues::issue`, or an inline member of a
                // `satisfies` block) "has no separately callable direct
                // entry" (conformances.md, "A conformer's name is its routed
                // entry"), so every invocation of its body is through the
                // requirement and the cast in it is authorized. A plain
                // machine bound by a selection row or binding expression
                // declares no edge and gets none: "its body was checked
                // without that authority, and a machine that relies on it
                // rejects at its own declaration".
                let edges = match self.typed.machine_by_symbol(machine) {
                    Some(declared) => self.typed.machine_trait_conformances(declared),
                    None => arena::RootRows::from_slice(&[]),
                };
                let routed = routes.iter().any(|route| match route {
                    DomainEstablishmentRoute::ExactMachine { machine: origin } => {
                        *origin == machine
                    }
                    DomainEstablishmentRoute::CheckedRequirement {
                        trait_definition,
                        requirement,
                    } => edges.iter().any(|edge| {
                        edge.symbol == *trait_definition && edge.requirement_symbol == *requirement
                    }),
                    // A boundary requirement "additionally requires provider
                    // selection and admission", which is not followed here.
                    DomainEstablishmentRoute::BoundaryRequirement { .. } => false,
                });
                // "Naming a machine is neither trait sealing nor an implicit
                // grant to other machines in its package." So when no route
                // is a boundary requirement and none admits this machine,
                // the cast is wrong in every program: "a cast cannot mint
                // it". With a boundary route the machine may yet be an
                // admitted provider, so that stays a refusal.
                let exact = !routes.iter().any(|route| {
                    matches!(route, DomainEstablishmentRoute::BoundaryRequirement { .. })
                });
                if !routed && exact {
                    return Err(reject(
                        id,
                        source,
                        format!(
                            "this cast adds {} in a machine that is not one of the domain's \
                             establishment routes: a domain with routes is added only inside a \
                             machine its declaration names, and a cast anywhere else cannot \
                             mint that evidence",
                            self.domain_named(domain),
                        ),
                    ));
                }
                if !routed {
                    return unserved("cast that adds a domain outside its establishment routes");
                }
            }
            DomainIssuance::Planned(policy) => {
                let refusal = refuse(
                    id,
                    Construct::Machine,
                    Unserved::Expression(
                        "cast that adds a domain whose issuers are in an access plan",
                    ),
                )
                .with_source_span(source.span());
                return Err(explain_policy_refusal(policy, refusal));
            }
        }
        Ok(())
    }
}

impl Program<'_> {
    /// Whether `expression` is a quoted byte literal, by the node typing
    /// recorded for it.
    fn byte_literal_operand(
        &self,
        expression: crate::source_state::expression::ExpressionHandle,
    ) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                crate::source_state::expression::ExpressionNode::String(_)
            )
    }

    fn array_literal_operand(
        &self,
        expression: crate::source_state::expression::ExpressionHandle,
    ) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                crate::source_state::expression::ExpressionNode::ArrayLiteral(_)
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::TypedTrees;
    use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
    use crate::source_state::types::TypeReferenceHandle;
    use language_core::{CastForm, ReferenceAccess};
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;

    fn plain() -> TableCastExpression {
        TableCastExpression {
            value: ExpressionHandle::invalid(),
            target_type: TypeReferenceHandle::invalid(),
            result_type: TypeReferenceHandle::invalid(),
            target_label: arena::HandleSpan::empty(),
            domain: ArithmeticDomain::Exact,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: SymbolHandle::invalid(),
            semantic_domain_id: semantic_vocabulary::SemanticDomainId::NULL,
            form: CastForm::Value,
        }
    }

    fn refused(cast: TableCastExpression) -> String {
        let mut typed = TypedTrees::default();
        let handle = typed.expression_table.insert(ExpressionNode::Cast(cast));
        let mut program = Program::new(&typed, false);
        let refusal = program
            .expression(MachineId::new(1).unwrap(), &[], handle, None)
            .unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        refusal.detail
    }

    /// Missing retained types and unserved domain combinations refuse before
    /// reading the operand; a recast needs its exact reference destination.
    #[test]
    fn casts_refuse_missing_types_and_unserved_domain_combinations_before_the_operand() {
        assert_eq!(
            refused(TableCastExpression {
                form: CastForm::Recast(ReferenceAccess::Shared),
                ..plain()
            }),
            "expression storage view outside reference formation"
        );
        assert_eq!(
            refused(TableCastExpression {
                semantic_domain_symbol: SymbolHandle::from_arena_index(3),
                ..plain()
            }),
            "expression cast naming a domain"
        );
        // The type route checks the carrier before applying any policy.
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Wrapping,
            ArithmeticDomain::Saturating,
            ArithmeticDomain::Trapping,
        ] {
            assert_eq!(
                refused(TableCastExpression { domain, ..plain() }),
                "type missing type"
            );
        }
    }
}
