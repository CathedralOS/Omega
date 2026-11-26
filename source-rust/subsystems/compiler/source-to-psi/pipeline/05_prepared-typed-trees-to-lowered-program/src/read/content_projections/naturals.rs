//! Normalize the total natural fragment using the ordinary typed clause reader for arithmetic.
use super::Program;
use crate::{Unserved, refuse};
use crate::source_state::data::DataMember;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use crate::source_state::types::TypeReferenceNode;
use language_core::core_vocabulary::CoreVocabulary;
use semantic_vocabulary::{ContentProjectionScalar, MachineId};
use terminal_check_tables::{Checked, Construct};

impl Program<'_> {
    pub(super) fn content_natural(
        &mut self,
        id: MachineId,
        expression: ExpressionHandle,
        subject: symbols::SymbolHandle,
        entry: symbols::SymbolHandle,
        carrier: crate::body::ValueType,
    ) -> Checked<ContentProjectionScalar> {
        let typed = self.typed;
        let table = &typed.expression_table;
        let unsupported = || {
            refuse(
                id,
                Construct::Machine,
                Unserved::Declaration(
                    "content magnitude or endpoint outside the supported closed proof-natural fragment",
                ),
            )
        };
        if !table.expression_is_valid(expression) {
            return Err(unsupported());
        }
        match table.expression(expression) {
            ExpressionNode::Name(name) if !name.static_application.is_valid() => {
                let variant = name.symbol;
                if !variant.is_valid() {
                    return Err(unsupported());
                }
                let nat = typed.symbols.get(variant).parent;
                if typed.core_vocabulary(nat) != Some(CoreVocabulary::ProofNatural) {
                    return Err(unsupported());
                }
                let data = typed
                    .unique_data_definition_by_symbol(nat)
                    .ok_or_else(unsupported)?;
                if !typed.data_members(data).iter().any(|member| matches!(member, DataMember::Variant(case)
                    if case.symbol == variant && case.name.as_str() == "Zero" && case.payload.is_empty())) {
                    return Err(unsupported());
                }
                Ok(ContentProjectionScalar::Natural("0".to_owned()))
            }
            ExpressionNode::StructLiteral(literal) => {
                if literal.zero_omitted_fields
                    || typed.core_vocabulary(literal.type_symbol)
                        != Some(CoreVocabulary::ProofNatural)
                {
                    return Err(unsupported());
                }
                if literal.application_type.is_valid()
                    && !matches!(typed.type_reference_table.type_reference(literal.application_type),
                        TypeReferenceNode::Named{symbol,..} if *symbol == literal.type_symbol)
                {
                    return Err(unsupported());
                }
                let data = typed
                    .unique_data_definition_by_symbol(literal.type_symbol)
                    .ok_or_else(unsupported)?;
                let case = typed
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(case) if Some(case.symbol) == literal.case_symbol => {
                            Some(case)
                        }
                        _ => None,
                    })
                    .ok_or_else(unsupported)?;
                let fields = table.struct_fields(literal.fields);
                match case.name.as_str() {
                    "Zero" if fields.is_empty() && case.payload.is_empty() => {
                        Ok(ContentProjectionScalar::Natural("0".to_owned()))
                    }
                    "Succ" => {
                        let Some([declared]) = typed.data_payload_fields(case).as_array() else {
                            return Err(unsupported());
                        };
                        let [written] = fields else {
                            return Err(unsupported());
                        };
                        if written.field_symbol != declared.symbol || written.name != declared.name
                        {
                            return Err(unsupported());
                        }
                        Ok(ContentProjectionScalar::Successor(Box::new(
                            self.content_natural(id, written.value, subject, entry, carrier)?,
                        )))
                    }
                    _ => Err(unsupported()),
                }
            }
            ExpressionNode::Cast(cast) => {
                let types = &typed.type_reference_table;
                let is_nat = |reference| {
                    types.contains_type_reference(reference)
                        && matches!(types.type_reference(reference), TypeReferenceNode::Named {symbol,..}
                    if typed.core_vocabulary(*symbol)==Some(CoreVocabulary::ProofNatural))
                };
                if cast.form != language_core::cast_form::CastForm::Value
                    || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
                    || !is_nat(cast.target_type)
                    || !is_nat(cast.result_type)
                    || !cast.semantic_domain.is_empty()
                    || !cast.semantic_domain_arguments.is_empty()
                    || cast.semantic_domain_symbol.is_valid()
                    || cast.semantic_domain_id != semantic_vocabulary::SemanticDomainId::NULL
                {
                    return Err(unsupported());
                }
                // Clause reading also judges ordinary pure helper calls. A content
                // projection only permits an explicit embed of a subject field.
                let mut terms = vec![cast.value];
                while let Some(term) = terms.pop() {
                    match table.expression(term) {
                        ExpressionNode::Binary(binary) => terms.extend([binary.left, binary.right]),
                        ExpressionNode::Integer(_) => (),
                        ExpressionNode::Call(call)
                            if typed
                                .symbols
                                .builtin_function_for_symbol(call.target_symbol)
                                == Some(symbols::BuiltinFunction::IntegerEmbed) =>
                        {
                            let [operand] = table.expression_handles(call.arguments) else {
                                return Err(unsupported());
                            };
                            let mut operand = *operand;
                            while let ExpressionNode::Member(member) = table.expression(operand) {
                                operand = member.receiver;
                            }
                            let ExpressionNode::Name(root) = table.expression(operand) else {
                                return Err(unsupported());
                            };
                            if root.head_symbol != subject || root.static_application.is_valid() {
                                return Err(unsupported());
                            }
                        }
                        _ => return Err(unsupported()),
                    }
                }
                let slots = [super::super::Slot {
                    symbol: subject,
                    value_type: carrier,
                    assignable: false,
                    homed: false,
                    erased: false,
                    borrowed: Some(language_core::ReferenceAccess::Shared),
                    borrowed_regions: crate::body::Regions::NONE,
                }];
                let scope = super::super::contracts::ClauseScope {
                    slots: &slots,
                    values: &[None],
                    bound: &[],
                    owner: entry,
                    result: super::super::contracts::ResultTerm::Absent(
                        "a projection body has no result in scope",
                    ),
                    subject: None,
                    formal_fields: true,
                    logical: None,
                };
                let term = self
                    .clause_term(
                        id,
                        &scope,
                        cast.value,
                        super::super::clause_terms::ClauseUse::Integer,
                    )?
                    .integer(id)?;
                self.nonnegative_content_term(
                    id,
                    &term,
                    carrier,
                    super::super::contracts::formal_place(id, &slots, 0)?,
                )
            }
            _ => Err(unsupported()),
        }
    }

    fn nonnegative_content_term(
        &self,
        id: MachineId,
        term: &semantic_vocabulary::IntegerMathTerm,
        carrier: crate::body::ValueType,
        subject: semantic_vocabulary::PlaceId,
    ) -> Checked<ContentProjectionScalar> {
        use crate::body::ValueType;
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, IntegerMathTerm, ScalarTerm, ScalarType,
        };
        let missing = || {
            refuse(
                id,
                Construct::Machine,
                Unserved::Declaration(
                    "content requires nonnegative proof arithmetic over u8/u16/u32/u64/address subject fields",
                ),
            )
        };
        let sub = |term| {
            self.nonnegative_content_term(id, term, carrier, subject)
                .map(Box::new)
        };
        Ok(match term {
            IntegerMathTerm::IntegerLiteral(literal) if !literal.negative() => {
                ContentProjectionScalar::Natural(literal.magnitude().to_string())
            }
            IntegerMathTerm::Add(a, b) => ContentProjectionScalar::Add(sub(a)?, sub(b)?),
            IntegerMathTerm::Multiply(a, b) => ContentProjectionScalar::Multiply(sub(a)?, sub(b)?),
            IntegerMathTerm::Embed(term) => {
                let (root, steps, integer) = match term.as_ref() {
                    ScalarTerm::IntegerField {
                        root,
                        path,
                        scalar_type,
                        ..
                    } => (*root, path.to_vec(), *scalar_type),
                    ScalarTerm::StructuralRead {
                        root,
                        path,
                        scalar_type: ScalarType::Integer(ty),
                    } => (
                        *root,
                        super::super::contracts::static_read_path(path).ok_or_else(missing)?,
                        *ty,
                    ),
                    _ => return Err(missing()),
                };
                if root != subject
                    || steps.is_empty()
                    || !integer.is_address()
                        && (integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                            || !matches!(integer.bits(), Some(8 | 16 | 32 | 64)))
                {
                    return Err(missing());
                }
                let mut value = carrier;
                let mut names = Vec::new();
                for step in steps {
                    let (
                        ValueType::Record(record, ..),
                        CanonicalStructuralPathSegment::Field(field),
                    ) = (value, step)
                    else {
                        return Err(missing());
                    };
                    let field = self
                        .data_type(id, record)?
                        .fields()
                        .iter()
                        .find(|part| part.id == field)
                        .ok_or_else(missing)?;
                    names.push(field.identity.clone());
                    value = field.value_type;
                }
                if !matches!(value, ValueType::Scalar(ScalarType::Integer(ty),_) if ty==integer) {
                    return Err(missing());
                }
                ContentProjectionScalar::RuntimeScalarEmbedding(names)
            }
            _ => return Err(missing()),
        })
    }
}
