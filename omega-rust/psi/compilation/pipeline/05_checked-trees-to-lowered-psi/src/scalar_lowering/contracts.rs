//! Scalar contracts over entry parameters and the normal-return result.
//! Ordinary, composed and boundary bodies share the same predicate meaning.
//! Only their execution schedules differ. Psi checking establishes authored
//! guarantees; lowering publishes their representable scalar consequences in
//! the declared result namespace, without replaying the source judgment.
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue, IntegerValue,
    LoweringError, Proposition, ScalarTerm, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, ValueDeclaration, unsupported,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::proofs::contract_predicates::{PredicateTerms, connective};
use crate::terminal_identities::{allocate_dense, value_id};
use checked_trees::{
    CheckedErasedProofParameterPlan, CheckedProofTerm, CheckedStructuralScalarParameterPlan,
    CheckedTrees,
};
use semantic_vocabulary::{ProofTerm, ProofTermField, ScalarType};
use terminal_psi::{ErasedProofFormal, StructuralResultDeclaration};

mod namespace;
pub(crate) use namespace::validate_indexed_operands;
mod result_range;
pub(crate) use result_range::with_result_range;

/// The structural namespace a closed scalar contract may observe: the
/// machine's `(source position, declaration)` parameter roster plus the
/// element-view carriers resolved from it. Scalar-only contexts pass
/// `ContractViewNamespace::EMPTY`.
#[derive(Clone, Copy)]
pub(crate) struct ContractViewNamespace<'a> {
    pub parameters: &'a [(u32, StructuralParameterDeclaration)],
    /// Normal guarantees observe the declared result place, never a synthetic
    /// entry parameter or a returned body-local place.
    pub result: Option<&'a StructuralResultDeclaration>,
    /// Structural type declarations the parameters' member paths resolve
    /// against; member leaves in contract operands fail closed when absent.
    pub structural_types: &'a [StructuralTypeDeclaration],
    pub element_views: &'a std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
}

static EMPTY_ELEMENT_VIEWS: std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>> =
    std::collections::BTreeMap::new();

impl ContractViewNamespace<'static> {
    /// Scalar-only contexts pass `ContractViewNamespace::EMPTY`.
    pub(crate) const EMPTY: ContractViewNamespace<'static> = ContractViewNamespace {
        parameters: &[],
        result: None,
        structural_types: &[],
        element_views: &EMPTY_ELEMENT_VIEWS,
    };
}

pub(crate) fn clauses(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    clauses: &[Option<ClosedScalarContractValue>],
    namespace: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
) -> Result<Option<Proposition>, LoweringError> {
    let mut combined = None;
    for clause in clauses {
        let proposition = match clause {
            Some(ClosedScalarContractValue::Predicate(predicate)) => {
                proposition(identities, predicate, namespace, erased, views)?
            }
            // The checked selection gate established builtin reflexivity.
            Some(ClosedScalarContractValue::Boolean(_) | ClosedScalarContractValue::Integer(_)) => {
                Proposition::Truth
            }
            // Floating entry ranges discharge through the retained scalar
            // qualification catalog, never as propositions; covered_requires
            // strips them before this conversion.
            Some(ClosedScalarContractValue::FloatRange(_)) => {
                return unsupported("scalar floating entry range is not a proposition clause");
            }
            Some(ClosedScalarContractValue::SourceCheckedGuarantee { .. }) => {
                return unsupported(
                    "source-checked guarantee has no runtime requirement projection",
                );
            }
            None => return unsupported("scalar contract clause has no checked predicate"),
        };
        combined = Some(if let Some(previous) = combined {
            connective(previous, proposition, true)?
        } else {
            proposition
        });
    }
    Ok(combined)
}

/// Only representable consequences become Terminal guarantees. Source checking
/// owns the other authored guarantees; absence of a scalar projection is not
/// a missing source proof or an assertion of runtime truth.
pub(crate) fn projected_guarantees(
    guarantees: &[Option<ClosedScalarContractValue>],
) -> impl Iterator<Item = &Option<ClosedScalarContractValue>> {
    guarantees.iter().filter(|guarantee| {
        !matches!(
            guarantee,
            None | Some(ClosedScalarContractValue::SourceCheckedGuarantee { .. })
        )
    })
}

/// Sort and deduplicate a published `requires` roster by the codec-owned
/// canonical proposition order. The derived `Ord` on `Proposition` follows
/// Rust declaration order, which is not the canonical byte order (a
/// field-rooted scalar term encodes after an integer literal while sorting
/// before it), so ordering the values directly still emits a roster the codec
/// rejects.
pub(crate) fn canonicalize_requires(requires: &mut Vec<Proposition>) -> Result<(), LoweringError> {
    let mut keyed = requires
        .drain(..)
        .map(|proposition| {
            terminal_codec::canonical_proposition_order_key(&proposition)
                .map(|key| (key, proposition))
                .map_err(|_| {
                    LoweringError::Unsupported(
                        "published requires proposition is not canonically encodable",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    requires.extend(keyed.into_iter().map(|(_, proposition)| proposition));
    Ok(())
}

/// Allocate terminal value declarations for a checked plan's proof-only erased
/// scalar formals, in authored order. Callers pass the same `next_value`
/// counter that allocated the dense scalar parameters so erased identities
/// stay dense right after them.
pub(crate) fn erased_formal_declarations(
    erased: &[CheckedStructuralScalarParameterPlan],
    next_value: &mut u64,
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    erased
        .iter()
        .map(|parameter| {
            Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(next_value)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect()
}

/// Emit the terminal erased-proof formal roster for one checked plan, in
/// authored order. Proof formals carry their semantic type identity rather
/// than a scalar type or value id — they never name a runtime operand.
pub(crate) fn erased_proof_formal_declarations(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    erased: &[CheckedErasedProofParameterPlan],
) -> Vec<ErasedProofFormal> {
    erased
        .iter()
        .map(|parameter| ErasedProofFormal {
            source_position: parameter.source_position,
            type_identity: identities.intern(
                &checked
                    .type_identity_text(parameter.type_identity)
                    .unwrap_or_default(),
            ),
        })
        .collect()
}

/// The lowering-stage proof term: identical in shape to the terminal
/// `ProofTerm`, except scalar payload leaves stay `LoweredDirectExpression`s
/// until the caller's `ValueDeclaration` namespace exists at emission.
/// `Formal` positions are already resolved against the caller's erased-proof
/// roster; only `Scalar` leaves still need `values`/`erased`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredProofTerm {
    Construction {
        type_identity: typed_trees::type_identity::TypeIdentityId,
        case_identity: Option<String>,
        fields: Vec<LoweredProofTermField>,
    },
    Formal {
        position: u32,
    },
    Scalar(LoweredDirectExpression),
}

/// One named payload field of a lowering-stage proof-term construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredProofTermField {
    pub(crate) field_identity: String,
    pub(crate) term: LoweredProofTerm,
}

/// Convert one checked proof-only actual into its lowering-stage term.
/// `Formal` occurrences resolve against the caller's erased-proof roster by
/// symbol, so the emitted position always indexes the roster carried by the
/// block or contract these terms appear under. Construction identities
/// canonicalize through the same `::` display path every other identity uses.
/// Scalar leaves stay checked expressions: their `ValueId`s belong to the
/// caller namespace the emitter supplies.
pub(crate) fn checked_proof_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    term: &CheckedProofTerm,
    caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
) -> Result<LoweredProofTerm, LoweringError> {
    match term {
        CheckedProofTerm::Construction {
            data_symbol: _,
            type_identity,
            case_symbol,
            fields,
        } => Ok(LoweredProofTerm::Construction {
            type_identity: *type_identity,
            case_identity: case_symbol.map(|symbol| checked.symbols.display_path(symbol, "::")),
            fields: fields
                .iter()
                .map(|field| {
                    Ok(LoweredProofTermField {
                        field_identity: checked.symbols.display_path(field.field_symbol, "::"),
                        term: checked_proof_term(
                            identities,
                            checked,
                            &field.term,
                            caller_erased_proof_parameters,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        }),
        CheckedProofTerm::Formal { parameter_symbol } => {
            let position = caller_erased_proof_parameters
                .iter()
                .position(|parameter| parameter.parameter_symbol == *parameter_symbol)
                .ok_or(LoweringError::Unsupported(
                    "proof actual references a formal outside the caller's erased-proof roster",
                ))?;
            Ok(LoweredProofTerm::Formal {
                position: u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported(
                        "erased-proof roster position exceeds the terminal term width",
                    )
                })?,
            })
        }
        CheckedProofTerm::Scalar(expression) => Ok(LoweredProofTerm::Scalar(
            crate::expression_preparation::prepare_expression::lower_checked_scalar_expression(
                identities,
                expression,
                &[],
            )?,
        )),
    }
}

/// Emit a lowering-stage proof term into its terminal form. `values` is the
/// caller's scalar namespace (parameters followed by in-scope locals, exactly
/// the ordering scalar actuals use) and `erased` the caller's erased scalar
/// declarations — the namespaces `ErasedParameter`/`Parameter`/`Local`
/// positions inside scalar leaves index.
pub(crate) fn lowered_proof_term(
    checked: &CheckedTrees,
    term: &LoweredProofTerm,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ProofTerm, LoweringError> {
    match term {
        LoweredProofTerm::Construction {
            type_identity,
            case_identity,
            fields,
        } => Ok(ProofTerm::Construction {
            type_identity: checked
                .type_identity_text(*type_identity)
                .unwrap_or_default(),
            case_identity: case_identity.clone(),
            fields: fields
                .iter()
                .map(|field| {
                    Ok(ProofTermField {
                        field_identity: field.field_identity.clone(),
                        term: lowered_proof_term(checked, &field.term, values, erased)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        }),
        LoweredProofTerm::Formal { position } => Ok(ProofTerm::Formal {
            position: *position,
        }),
        LoweredProofTerm::Scalar(expression) => Ok(ProofTerm::Scalar(
            crate::proofs::crash_routes::lowered_direct_scalar_term(expression, values, erased)?,
        )),
    }
}

/// Preserve integer contract relations as propositions, not executable Boolean
/// comparisons equated with true. Call composition can then cite the exact
/// relation in ordinary fixed-integer proof rules.
pub(crate) fn proposition(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    predicate: &CheckedBooleanExpression,
    namespace: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
) -> Result<Proposition, LoweringError> {
    self::namespace::validate(predicate)?;
    crate::proofs::contract_predicates::proposition(
        identities,
        predicate,
        &ScalarContractTerms {
            namespace,
            erased,
            views: *views,
        },
    )
}

struct ScalarContractTerms<'namespace> {
    namespace: &'namespace [ValueDeclaration],
    erased: &'namespace [ValueDeclaration],
    views: ContractViewNamespace<'namespace>,
}

impl PredicateTerms for ScalarContractTerms<'_> {
    fn case_membership(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
    ) -> Result<Proposition, LoweringError> {
        let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression
        else {
            return unsupported("closed case predicate has no structural subject");
        };
        let parameters = self.authored_structural_parameters();
        crate::proofs::crash_routes::lower_structural_case_membership(
            identities,
            subject,
            case,
            &parameters,
            self.views.result,
            self.views.structural_types,
        )
    }

    fn scalar(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        // Integer operations compose through the shared predicate lowering
        // entry requirements use, so a member leaf below arithmetic
        // (`self.length - 1`) resolves in the same namespace as a bare one.
        // Closed contracts carry no partial operation's safety evidence.
        crate::proofs::integer_predicates::lower(expression, &[], &mut |leaf| {
            self.leaf(identities, leaf)
        })
    }

    fn boolean(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => self.structural_member_term(
                identities,
                *parameter_position,
                path,
                ScalarType::Boolean,
            ),
            CheckedBooleanExpression::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                let position = root.parameter().ok_or(LoweringError::Unsupported(
                    "closed byte predicate requires a structural formal",
                ))?;
                let parameters = self.authored_structural_parameters();
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.position == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte predicate names a non-structural formal",
                    ))?;
                let (root, path, carrier) = if path.is_empty() {
                    let carrier = terminal_psi::find_structural_type(
                        &self.views.structural_types[..],
                        parameter.structural_type,
                    )
                    .and_then(|declaration| match declaration.shape {
                        terminal_psi::StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                        _ => None,
                    });
                    (parameter.place, Vec::new(), carrier)
                } else {
                    let (root, path, field) =
                        crate::proofs::crash_routes::lower_structural_member_path(
                            identities,
                            position,
                            path,
                            &parameters,
                            self.views.structural_types,
                        )?;
                    (root, path, field.byte_sequence_carrier())
                };
                let readable = match carrier {
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => true,
                    Some(terminal_psi::ByteSequenceCarrier::BorrowedView { access }) => {
                        matches!(
                            access.unwrap_or(parameter.access),
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                    }
                    _ => false,
                };
                if !readable || parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
                {
                    return unsupported("byte predicate formal is not a readable byte carrier");
                }
                Ok(ScalarTerm::ByteSequencePredicate {
                    predicate: *predicate,
                    root,
                    path,
                })
            }
            CheckedBooleanExpression::Not(operand) => {
                ScalarTerm::boolean_not(self.boolean(identities, operand)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                self.boolean(identities, left)?,
                self.boolean(identities, right)?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            _ => crate::proofs::crash_routes::checked_boolean_scalar_term(
                identities,
                expression,
                self.namespace,
                self.erased,
            ),
        }
    }

    fn strict_bound(&self, left: ScalarTerm, right: ScalarTerm) -> Proposition {
        strict_result_bound(left, right)
    }
}

impl ScalarContractTerms<'_> {
    fn structural_member_term(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, LoweringError> {
        let parameters = self.authored_structural_parameters();
        crate::proofs::crash_routes::lower_structural_member_term(
            identities,
            parameter_position,
            path,
            scalar_type,
            &parameters,
            self.views.structural_types,
        )
    }

    // Checked leaves use authored source positions, while Terminal separates
    // scalar and structural parameter lanes. Rejoin by the retained source key,
    // never by a coincidentally equal dense structural ordinal.
    fn authored_structural_parameters(&self) -> Vec<StructuralParameterDeclaration> {
        self.views
            .parameters
            .iter()
            .map(|(position, declaration)| StructuralParameterDeclaration {
                position: *position,
                ..declaration.clone()
            })
            .collect()
    }

    fn leaf(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        if let CheckedScalarExpression::Boolean(predicate) = expression {
            return self.boolean(identities, predicate);
        }
        if matches!(
            expression,
            CheckedScalarExpression::StructuralParameterIndexedRead { .. }
        ) {
            return crate::proofs::crash_routes::lower_structural_indexed_term(
                identities,
                expression,
                &self.authored_structural_parameters(),
                self.views.structural_types,
                &mut |identities, selector| self.scalar(identities, selector),
            );
        }
        if let CheckedScalarExpression::StructuralSequenceLength { root, path } = expression {
            let path = checked_trees::CheckedScalarReadProjection::static_path(path).ok_or(
                LoweringError::Unsupported(
                    "closed sequence extent cannot discard runtime selectors",
                ),
            )?;
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "closed sequence extent requires a structural formal",
            ))?;
            return crate::proofs::crash_routes::lower_sequence_extent_term(
                identities,
                position,
                &path,
                &self.authored_structural_parameters(),
                self.views.structural_types,
            );
        }
        // Member leaves root at the contract's structural parameters and
        // mint field-rooted scalar terms; the entry namespace carries the
        // declarations and types the path resolves against.
        if let CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } = expression
        {
            // The roster keys each declaration by its authored source
            // position — the coordinate checked member operands name — so the
            // lookup copies carry that position rather than the block-dense
            // one the emitted block parameters use.
            // IEEE leaves keep their exact format, as entry requirements do.
            return self.structural_member_term(
                identities,
                *parameter_position,
                path,
                crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?,
            );
        }
        crate::proofs::crash_routes::checked_scalar_term_with_views(
            identities,
            expression,
            self.namespace,
            self.erased,
            self.views.parameters,
            self.views.element_views,
        )
    }
}

/// Integer strict bounds with a literal endpoint have an exact inclusive
/// spelling. Emit that spelling before any obligation is reconstructed; no
/// certificate is allowed to reinterpret a strict fact as an inclusive one.
fn strict_result_bound(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    let neighbor = |term: &ScalarTerm, increment: bool| {
        let ScalarTerm::Integer { scalar_type, value } = term else {
            return None;
        };
        let one = match value {
            IntegerValue::Signed(_) => IntegerValue::Signed(1),
            IntegerValue::Unsigned(_) => IntegerValue::Unsigned(1),
        };
        let value = if increment {
            scalar_type.exact_add(*value, one)
        } else {
            scalar_type.exact_sub(*value, one)
        }?;
        Some(ScalarTerm::Integer {
            scalar_type: *scalar_type,
            value,
        })
    };
    if let Some(endpoint) = neighbor(&right, false) {
        Proposition::LessOrEqual(left, endpoint)
    } else if let Some(endpoint) = neighbor(&left, true) {
        Proposition::LessOrEqual(endpoint, right)
    } else {
        // At an endpoint with no representable neighbor retain the original
        // proposition. In particular, never wrap x < MIN into x <= MAX.
        Proposition::LessThan(left, right)
    }
}

/// The erased-proof roster the caller machine's given state exposes to its
/// lowered terms: that state's row in the caller's published Unit body.
pub(crate) fn caller_erased_proof_roster(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> Result<&[CheckedErasedProofParameterPlan], LoweringError> {
    crate::unit::attached_unit::bodies::UnitBody::find(
        crate::unit::attached_unit::bodies::UnitPlans::published(&checked.facts.flow),
        machine,
    )?
    .state_erased_proof_parameters(state)
}
