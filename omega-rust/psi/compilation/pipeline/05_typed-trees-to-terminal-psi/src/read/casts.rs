/*
Keep carrier conversion separate from changes to a value's qualification.
Domain-only casts preserve the same value and carrier, with shared accounting
for executable expressions and contract terms. Adding a domain owes its issuer
and predicate checks. Removal keeps ownership and cannot erase access, cleanup
or borrowed restoration obligations.

Carrier casts start with an unqualified operand and perform an exact scalar
conversion before adding a policy. Float widening uses the authenticated Core
conversion whose entire input format is exactly representable in its result.
Trapping conversions, storage recasts and unserved anonymous operands refuse.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::TableCastExpression;

use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, Policy, Qualification, ValueType};
use crate::{SourceOrigin, Unserved, refuse, reject};

impl Program<'_> {
    pub(super) fn cast(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        cast: &TableCastExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        if let Some(entry) = cast.form.entry() {
            return self.domain_entry(id, slots, cast, entry, source);
        }
        if cast.form.is_recast() {
            return unserved("storage view outside reference formation");
        }
        if cast.semantic_domain_symbol.is_valid() || !cast.semantic_domain.is_empty() {
            return unserved("cast naming a domain");
        }
        // The typed trees hold `value as T in Wrapping` as the carrier `T`
        // with the policy beside it (`cast.domain`); a target written as a
        // constrained type goes through the type route like any other.
        use numerics::arithmetic::ArithmeticDomain;
        let named = match cast.domain {
            ArithmeticDomain::Exact => None,
            ArithmeticDomain::Wrapping => Some(Policy::Wrapping),
            ArithmeticDomain::Saturating => Some(Policy::Saturating),
            ArithmeticDomain::Trapping => return unserved("cast into Trapping"),
        };
        let target = match (self.value_type(id, cast.target_type)?, named) {
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
        // An anonymous operand has no carrier of its own; landing it is the
        // numeric landing rules' work, outside this family. The one such
        // operand read here is a literal cast into a policy: `5 as u32 in
        // Wrapping` is `(5 as u32)` with the policy added (domains.md,
        // "Carrier casts": "The cast may add domains to the result"), so
        // the literal lands in the carrier the cast names, in range or not
        // at all (numeric_values.md: no policy destination can "repair an
        // out-of-range initial value").
        let landing = if named.is_some() && self.wholly_anonymous(cast.value) {
            target.scalar()
        } else {
            None
        };
        let operand = self.expression(id, slots, cast.value, landing.map(ValueType::plain))?;
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
            use language_semantics::numeric_intrinsics::{NumericIntrinsic, NumericType};
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
                let clause =
                    semantic_vocabulary::FloatMeaningClause::conversion_roundtrip(source_term, to)
                        .map_err(|detail| {
                            refuse(id, Construct::Machine, Unserved::Expression(detail))
                        })?;
                Expression {
                    result: operand.result,
                    kind: ExpressionKind::Establish {
                        operand: Box::new(operand),
                        clauses: vec![semantic_vocabulary::Proposition::FloatMeaningEqual(clause)],
                        stated: format!(
                            "the float value is exactly representable in {}",
                            match to {
                                semantic_vocabulary::IeeeFloatFormat::Binary32 => "f32",
                                semantic_vocabulary::IeeeFloatFormat::Binary64 => "f64",
                            }
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
        meaning: language_semantics::numeric_intrinsics::NumericIntrinsic,
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
                source.authored_span().unwrap_or_default(),
                |symbol| {
                    self.typed.symbols.sealed_intrinsic(symbol)
                        == Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement)
                        && self.typed.numeric_intrinsic_execution(symbol) == Some(meaning)
                },
            ).ok_or_else(|| unavailable("exact carrier conversion has no canonical declaration"))?;
        let boundary = self.boundary_declaration(id, target)?;
        let signature = self
            .boundary_signatures
            .get(boundary)
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
                callee: semantic_vocabulary::TargetMachineCallable::Boundary(boundary),
                required_contract: None,
                source_call: None,
                static_reach_binding: None,
                arguments: vec![operand],
                designated: None,
            },
            result: signature.result,
            source,
        })
    }
}

impl Program<'_> {
    /// One entry of a domain cast, `value as =Keep -Remove +Add`
    /// (`wiki/spec/language/domains.md`, "Domain-only casts"). Typing
    /// accounted for the whole cast against the operand's declared type and
    /// left this entry with the type after it. The reader types the operand
    /// itself, so it does not take that accounting on trust: the entry must
    /// do to the operand's type exactly what its sign says.
    ///
    /// | Entry | The operand's type | The result's type |
    /// | `=D` | carries `D` | carries `D` |
    /// | `-D` | carries `D` | does not carry `D` |
    /// | `+D` | does not carry `D` | carries `D` |
    ///
    /// The value is never changed: "`as` never calls user code, rounds,
    /// saturates, traps".
    fn domain_entry(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        // A cast typing deferred is a row on its machine, raised before any
        // body is read; an entry without its type here was never accounted.
        if !cast.result_type.is_valid() {
            return unserved("domain cast typing did not account for");
        }
        // The entry preserves the operand's lifetime (domains.md, "Mutable
        // views": "The cast preserves the referent, lifetime and access
        // mode"). One typing left elided, a quoted literal's, is a region
        // variable its use decides, as a literal takes the lifetime its use
        // requires.
        let elision = std::mem::replace(&mut self.elision, super::regions::Elision::Variables);
        let result = self.qualified_view_type(id, cast.result_type);
        self.elision = elision;
        let result = result?;
        // A literal lands in the carrier of the type the cast gives it
        // (domains.md, "Literals and expected types": "An expected type
        // selects the carrier in which an anonymous literal lands"). A
        // quoted literal's carrier is the view the cast qualifies, and an
        // array literal's is the array: `[65u8, 66u8] as +Text` is a plain
        // `[u8; 2]` given the domain.
        let landing = if self.wholly_anonymous(cast.value) {
            result.scalar().map(ValueType::plain)
        } else if self.byte_literal_operand(cast.value) || self.array_literal_operand(cast.value) {
            result.qualified(Qualification::NONE)
        } else {
            None
        };
        let operand = self.expression(id, slots, cast.value, landing)?;
        self.entry_applied(id, cast, entry, operand, result, source)
    }

    /// `&text as +D`, as `borrow_value` forms it: `lent` is the reference
    /// to `text`, and the entry changes the domains it displays on its
    /// referent to `after`, keeping its referent, lifetime and access mode
    /// (domains.md, "Mutable views").
    pub(super) fn reference_entry(
        &mut self,
        id: MachineId,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        lent: Expression,
        after: Qualification,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let Some(result) = lent.result.and_then(|formed| formed.qualified(after)) else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("reference cast of a value that is not a reference"),
            ));
        };
        self.entry_applied(id, cast, entry, lent, result, source)
    }

    /// What one entry does to an operand already read, `result` being the
    /// type after it.
    fn entry_applied(
        &mut self,
        id: MachineId,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
        operand: Expression,
        result: ValueType,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use language_core::cast_form::CastEntryKind;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let Some(operand_type) = operand.result else {
            return unserved("domain cast of a Unit value");
        };
        // The view the entry gives is the operand's, at the operand's
        // region: the result type was read for its carrier and domains.
        let result = match (result, operand_type) {
            (ValueType::Record(row, qualification, _), ValueType::Record(_, _, regions)) => {
                ValueType::Record(row, qualification, regions)
            }
            _ => result,
        };
        let domain = self.domain_entry_change(id, operand_type, result, cast, entry)?;
        if entry.kind == CastEntryKind::Keep {
            return Ok(operand);
        }
        if let Some(domain) = domain {
            match entry.kind {
                CastEntryKind::Add => {
                    self.issue(id, domain, source)?;
                    // "Has predicates | Prove them for this value."
                    let (clauses, stated) = match (
                        result,
                        crate::body::DomainSet::EMPTY.with(domain),
                    ) {
                        (ValueType::Scalar(carrier, _), Some(domains)) => self.cast_predicates(
                            id,
                            Qualification {
                                policy: None,
                                domains,
                            },
                            carrier,
                        )?,
                        // `"text" as +Utf8`: a primitive byte predicate is
                        // decided by the bytes the view shows, and a view in
                        // a domain is served only as a data field's type
                        // (`view_type`), so this value is stored into
                        // such a field or dropped. The checker judges each
                        // store into a predicate field from the octets of
                        // the literal it supplies (byte_views.md: "Checking
                        // rejoins the source's declared predicates or
                        // literal contents"), so the cast states no clause.
                        // Its operand is a quoted literal, the one source
                        // whose bytes that judgment reads.
                        (ValueType::Record(row, _, _), Some(_))
                            if self.is_view(id, row)?
                                && typed_trees::byte_predicates::domain_byte_predicate(
                                    self.typed,
                                    domain.declaration,
                                )
                                .is_some() =>
                        {
                            if !matches!(operand.kind, ExpressionKind::ByteLiteral(_)) {
                                return unserved(
                                    "cast that adds a byte predicate to other than a quoted literal",
                                );
                            }
                            (Vec::new(), String::new())
                        }
                        // An array or record, or a reference displaying a
                        // domain on its referent (`&text as +Terminated`):
                        // the clauses are stated over the plain value at one
                        // structural subject, which lowering binds to the
                        // cast's result (`record_cast_predicates`).
                        (ValueType::Record(..), Some(domains))
                            if super::authority::has_predicates(
                                self.domain_theory(id, domain)?,
                            ) =>
                        {
                            self.record_cast_predicates(
                                id,
                                Qualification {
                                    policy: None,
                                    domains,
                                },
                                result,
                            )?
                        }
                        _ if super::authority::has_predicates(self.domain_theory(id, domain)?) => {
                            return unserved(
                                "cast that adds a domain with predicates to a value of a type parameter",
                            );
                        }
                        _ => (Vec::new(), String::new()),
                    };
                    if !clauses.is_empty() {
                        return Ok(Expression {
                            call_context: None,
                            kind: ExpressionKind::Establish {
                                operand: Box::new(operand),
                                clauses,
                                stated,
                            },
                            result: Some(result),
                            source,
                        });
                    }
                }
                _ => {
                    // `("text" as +Utf8) as -Utf8`: the predicate a literal
                    // was given is judged only where a field stores it
                    // (`domain_entry`, the `Add` arm), so removing it again
                    // before any store would leave it judged nowhere.
                    let mut beneath: &Expression = &operand;
                    while let ExpressionKind::Requalify(inner) = &beneath.kind {
                        beneath = inner.as_ref();
                    }
                    if !std::ptr::eq(beneath, &operand)
                        && matches!(beneath.kind, ExpressionKind::ByteLiteral(_))
                    {
                        return unserved(
                            "removal of a byte predicate a quoted literal was just given",
                        );
                    }
                    self.domain_removal(id, domain, operand_type)?;
                }
            }
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
    pub(super) fn domain_entry_change(
        &self,
        id: MachineId,
        operand: ValueType,
        result: ValueType,
        cast: &TableCastExpression,
        entry: language_core::cast_form::CastEntry,
    ) -> Checked<Option<crate::body::DomainInstance>> {
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
            if self
                .typed
                .domain_definition(cast.semantic_domain_symbol)
                .is_some()
                && !before.domains.contains(crate::body::DomainInstance {
                    declaration: cast.semantic_domain_symbol,
                    identity: cast.semantic_domain_id,
                })
            {
                return Err(mistyped("kept domain that the operand does not carry"));
            }
            return Ok(None);
        }
        let (wider, narrower) = match entry.kind {
            CastEntryKind::Remove => (before, after),
            CastEntryKind::Add => (after, before),
            CastEntryKind::Keep => return Err(mistyped("kept domain without an unchanged type")),
        };
        let mut named = wider
            .domains
            .members()
            .filter(|domain| !narrower.domains.contains(*domain));
        let domain = named.next();
        let one_member = named.next().is_none()
            && narrower.domains.is_subset_of(&wider.domains)
            && match (before.policy != after.policy, domain) {
                (true, None) => wider.policy.is_some() && narrower.policy.is_none(),
                (false, Some(_)) => true,
                _ => false,
            };
        if !one_member {
            return Err(mistyped(
                "domain cast entry that does not change exactly the domain it names",
            ));
        }
        if domain.is_some_and(|domain| {
            domain.declaration != cast.semantic_domain_symbol
                || domain.identity != cast.semantic_domain_id
        }) {
            return Err(mistyped(
                "changed domain application is not the one the entry selected",
            ));
        }
        Ok(domain)
    }

    /// Issuance restricts adding a domain, not forgetting its displayed
    /// qualification. Removal keeps value facts and the carrier's custody;
    /// access, cleanup and borrowed restoration obligations cannot disappear.
    pub(super) fn domain_removal(
        &self,
        id: MachineId,
        domain: crate::body::DomainInstance,
        operand: ValueType,
    ) -> Checked<()> {
        let unavailable = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let definition = self.domain_theory(id, domain)?;
        if definition.access_policy.is_some() {
            return Err(unavailable(
                "removal of a domain governed by an access policy",
            ));
        }
        if definition.cleanup_selection.is_some() {
            return Err(unavailable("removal of a domain with cleanup obligations"));
        }
        if definition.issuance() != typed_trees::domain::DomainIssuance::Unrestricted
            && let ValueType::Record(row, _, _) = operand
            && matches!(
                self.data_type(id, row)?.shape,
                crate::StorageShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::MutableBorrow)
                    }
                )
            )
        {
            return Err(unavailable(
                "domain removal from an exclusive view needs restoration proof",
            ));
        }
        Ok(())
    }

    /// Whether the machine being read may write `as +D` for the declared
    /// domain `domain`, by who the declaration says may issue it
    /// (`DomainDefinition::issuance`). This is the one place that judgment
    /// is made. domains.md, "Domain-only casts":
    ///
    /// | Domain | Obligation of `+D` |
    /// | Has predicates | Prove them for this value. |
    /// | Has neither predicates nor establishment routes | None beyond carrier validity |
    /// | Has establishment routes | Retain exact existing or authorized evidence; a cast cannot mint it. |
    ///
    /// A route names "an exact trait requirement or concrete machine as an
    /// alternative authorized origin", and "An exact machine is authorized
    /// during its invocation": the machine whose body holds the cast is one
    /// of the routes, or the cast is rejected. A requirement route
    /// authorizes "the invocation through that requirement and its selected
    /// implementation": the machine that declares that requirement's edge.
    /// A policy states its issuers in its plan. Build-time evaluation runs
    /// the plan of each demanded domain and records its routes, which this
    /// reads as `DomainIssuance::Only`; a plan nobody has run
    /// (`DomainIssuance::Planned`) shows nobody to be an issuer.
    ///
    /// Authorization proves no predicate ("Policy authorization cannot
    /// prove those predicates"): the cast owes the domain's predicates of
    /// its operand whoever issues it (`domain_entry`, `predicates`).
    fn issue(
        &mut self,
        id: MachineId,
        domain: crate::body::DomainInstance,
        source: SourceOrigin,
    ) -> Checked<()> {
        use language_semantics::DomainEstablishmentRoute;
        use typed_trees::domain::DomainIssuance;
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
                    None => &[],
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
            DomainIssuance::Planned(_) => {
                return unserved("cast that adds a domain whose issuers are in an access plan");
            }
        }
        Ok(())
    }
}

impl Program<'_> {
    /// Whether `expression` is a quoted byte literal, by the node typing
    /// recorded for it.
    fn byte_literal_operand(&self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::String(_)
            )
    }

    fn array_literal_operand(&self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::ArrayLiteral(_)
            )
    }

    /// Whether the record row `row` is a shared byte view.
    fn is_view(&self, id: MachineId, row: semantic_vocabulary::StructuralTypeId) -> Checked<bool> {
        Ok(matches!(
            self.data_type(id, row)?.shape,
            crate::StorageShape::ByteSequence(carrier) if carrier.is_shared_readable_view()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_core::{CastForm, ReferenceAccess};
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::TypeReferenceHandle;

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
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
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
        // A cast into Trapping is refused before its target is read.
        assert_eq!(
            refused(TableCastExpression {
                domain: ArithmeticDomain::Trapping,
                ..plain()
            }),
            "expression cast into Trapping"
        );
        // A plain cast with no target type is refused by the type route,
        // and so is one into Wrapping or Saturating: the policy is read,
        // then the carrier it qualifies.
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Wrapping,
            ArithmeticDomain::Saturating,
        ] {
            assert_eq!(
                refused(TableCastExpression { domain, ..plain() }),
                "type missing type"
            );
        }
    }
}
