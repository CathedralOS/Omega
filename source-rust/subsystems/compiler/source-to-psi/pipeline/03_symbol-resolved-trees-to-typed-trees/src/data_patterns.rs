//! Every record pattern keeps the record and fields typing selected for it.
//! A positional row names a place, and `..` waives the remaining fields;
//! typing resolves those and named rows against the same declared record.
//! Later stages read those identities without looking the names up again.
//!
//! `wiki/spec/language/patterns.md`, "Data and case decomposition": "A
//! positional pattern binds fields by position; binding names need not match
//! declaration names. These positions are semantic declaration order", and
//! "positional patterns have the corresponding exact arity. An explicit `..`
//! may waive the remaining components."
//!
//! A positional or rest pattern whose subject has no record known here is
//! refused: no later stage could still give a place its field.

use crate::source_resolved as resolved;
use crate::expressions::call_results::peel;
use crate::expressions::expression::table::lowerer::resolved_data_definition_for_type;
use crate::expressions::expression::table::member_binding::declared_type;
use crate::lowerer::exact_field_symbol;
use diagnostics::Diagnostic;
use language_core::PatternSelector;
use resolved::data::{DataDefinition, DataField, DataMember};
use resolved::expression::{ExpressionHandle, ExpressionTable, MatchFieldAction, TableMatchField};
use resolved::types::TypeReference;
use source_text::SourceSpan;

/// A record and its fields in declaration order, which is the order a
/// positional pattern counts in.
pub(crate) struct Record<'program> {
    pub(crate) definition: &'program DataDefinition,
    pub(crate) fields: Vec<&'program DataField>,
}

/// Why a subject has no fields for a pattern to select.
pub(crate) enum NoRecord<'program> {
    /// The subject's type is not declared where the pattern stands.
    UnknownType,
    /// The subject is a scalar, a collection or another value with no fields.
    NotData,
    /// The subject is a sum, which is matched by case.
    Sum(&'program DataDefinition),
}

/// The record `subject` holds, read from the declaration of the value it
/// names. References and domain constraints do not change the record.
pub(crate) fn subject_record<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
) -> Result<Record<'program>, NoRecord<'program>> {
    subject_record_within(program, expressions, subject, 0)
}

/// `subject_record` for a caller already `depth` steps into one chain of
/// declared types, which keeps a cyclic initializer from recursing forever.
pub(crate) fn subject_record_within<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
    depth: usize,
) -> Result<Record<'program>, NoRecord<'program>> {
    let declared =
        declared_type(program, expressions, subject, depth).ok_or(NoRecord::UnknownType)?;
    declared_record(program, declared)
}

/// The record a declared type names.
pub(crate) fn declared_record<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    declared: &'program TypeReference,
) -> Result<Record<'program>, NoRecord<'program>> {
    let carrier = peel(program, declared);
    let Some(definition) = resolved_data_definition_for_type(program, carrier) else {
        let named = match carrier {
            TypeReference::Named { symbol, .. } => Some(*symbol),
            TypeReference::Generic(application) => Some(application.base_symbol),
            _ => None,
        };
        // A data type whose definition is not in reach may still be a
        // record. A builtin, a type parameter or a collection never is.
        return Err(match named {
            Some(symbol)
                if !symbol.is_valid()
                    || program.symbols.get(symbol).kind == symbols::SymbolKind::Data =>
            {
                NoRecord::UnknownType
            }
            _ => NoRecord::NotData,
        });
    };
    defined_record(program, definition)
}

/// The fields of one data definition, when it declares fields and no case.
pub(crate) fn defined_record<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    definition: &'program DataDefinition,
) -> Result<Record<'program>, NoRecord<'program>> {
    // Two definitions under one symbol give a place no single field.
    if program
        .data_definitions
        .iter()
        .filter(|other| other.symbol == definition.symbol)
        .count()
        != 1
    {
        return Err(NoRecord::UnknownType);
    }
    let mut fields = Vec::new();
    for member in program.data_members(definition.members) {
        match member {
            DataMember::Field(field) if exact_field_symbol(program, definition.symbol, field) => {
                fields.push(field);
            }
            DataMember::Field(_) => return Err(NoRecord::UnknownType),
            DataMember::Variant(_) => return Err(NoRecord::Sum(definition)),
        }
    }
    Ok(Record { definition, fields })
}

/// The field at `position` of the record `receiver` holds, for the read a
/// positional pattern generates. `at` is where the pattern wrote that place.
pub(crate) fn positional_field<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    position: u32,
    at: SourceSpan,
) -> Result<&'program DataField, Diagnostic> {
    let record = subject_record(program, expressions, receiver)
        .map_err(|reason| no_positional_decomposition(expressions, receiver, reason, at))?;
    record
        .fields
        .get(position as usize)
        .copied()
        .ok_or_else(|| too_many_components(&record, position as usize + 1, at))
}

/// One row of a pattern after its subject's record gave it a field.
pub(crate) struct FieldRow<'program> {
    pub(crate) field: &'program DataField,
    pub(crate) action: MatchFieldAction,
    /// False for a field the pattern did not write and its `..` waives.
    pub(crate) written: bool,
}

pub(crate) struct FieldRows<'program> {
    pub(crate) record: symbols::SymbolHandle,
    pub(crate) fields: Vec<FieldRow<'program>>,
}

/// The rows of a pattern over `subject`, each naming one declared field.
///
/// Named case payloads keep their existing case-selection path. A named
/// subject whose record is not known here stays unresolved and grants no
/// record identity. An explicit `let` head can supply its declared record;
/// lowering still checks that the actual subject holds that record.
pub(crate) fn field_rows<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
    rows: &[TableMatchField],
    head: Option<Record<'program>>,
) -> Result<Option<FieldRows<'program>>, Diagnostic> {
    let names_only = rows.iter().all(|row| row.selector == PatternSelector::Name);
    let whole = written_span(rows, expressions.source_span(subject));
    let positional = rows
        .iter()
        .any(|row| matches!(row.selector, PatternSelector::Position(_)));
    let record = head.map_or_else(|| subject_record(program, expressions, subject), Ok);
    let record = match record {
        Ok(record) => record,
        Err(_) if names_only => return Ok(None),
        Err(reason) => {
            return Err(if positional {
                no_positional_decomposition(expressions, subject, reason, whole)
            } else {
                no_rest(expressions, subject, reason, whole)
            });
        }
    };
    let mut selected = vec![false; record.fields.len()];
    let mut resolved = Vec::with_capacity(record.fields.len());
    let mut components = 0usize;
    let mut rest = None;
    for row in rows {
        match row.selector {
            PatternSelector::Position(position) => {
                components += 1;
                let Some(field) = record.fields.get(position as usize).copied() else {
                    let written = rows
                        .iter()
                        .filter(|row| matches!(row.selector, PatternSelector::Position(_)))
                        .count();
                    return Err(too_many_components(
                        &record,
                        written,
                        row.member.source_span(),
                    ));
                };
                selected[position as usize] = true;
                resolved.push(FieldRow {
                    field,
                    action: row.action.clone(),
                    written: true,
                });
            }
            PatternSelector::Name => {
                let mut named = record
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.name.as_str() == row.member.as_str());
                let Some((position, field)) = named.next() else {
                    return Err(Diagnostic::error(format!(
                        "`{}` has no field `{}` for this pattern to name",
                        record.definition.name.as_str(),
                        row.member.as_str()
                    ))
                    .with_source_span(row.member.source_span()));
                };
                if named.next().is_some() {
                    return Err(Diagnostic::error(format!(
                        "`{}` has more than one field `{}` for this pattern to name",
                        record.definition.name.as_str(),
                        row.member.as_str()
                    ))
                    .with_source_span(row.member.source_span()));
                }
                selected[position] = true;
                resolved.push(FieldRow {
                    field,
                    action: row.action.clone(),
                    written: true,
                });
            }
            PatternSelector::Rest => rest = Some(row),
        }
    }
    match rest {
        // `..` is the last row: it waives every field no earlier row took.
        Some(_) => {
            for (field, taken) in record.fields.iter().zip(&selected) {
                if !taken {
                    resolved.push(FieldRow {
                        field,
                        action: MatchFieldAction::Waive,
                        written: false,
                    });
                }
            }
        }
        None if positional && components != record.fields.len() => {
            return Err(Diagnostic::error(format!(
                "this pattern has {}, but `{}` has {}: give every field a place in declaration \
                 order, or end the pattern with `..` to waive the rest",
                counted(components, "component"),
                record.definition.name.as_str(),
                counted(record.fields.len(), "field"),
            ))
            .with_source_span(whole));
        }
        None => {}
    }
    Ok(Some(FieldRows {
        record: record.definition.symbol,
        fields: resolved,
    }))
}

/// The span from a pattern's first written row to its last.
fn written_span(rows: &[TableMatchField], otherwise: SourceSpan) -> SourceSpan {
    let mut spans = rows
        .iter()
        .map(|row| row.member.source_span())
        .filter(|span| span.span.start != span.span.end);
    let Some(first) = spans.next() else {
        return otherwise;
    };
    let last = spans.next_back().unwrap_or(first);
    if first.source_id != last.source_id {
        return first;
    }
    SourceSpan::new(
        first.source_id,
        source_text::Span::new(first.span.start, last.span.end),
    )
}

/// The subject as its author wrote it. A dispatch reads its subject through
/// a local the compiler made, and that local keeps the authored expression.
fn shown(expressions: &ExpressionTable, subject: ExpressionHandle) -> String {
    let authored = expressions.materialization_source(subject);
    expressions.display_name(if authored.is_valid() {
        authored
    } else {
        subject
    })
}

fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn too_many_components(record: &Record<'_>, written: usize, at: SourceSpan) -> Diagnostic {
    Diagnostic::error(format!(
        "this pattern has {}, but `{}` has only {}",
        counted(written, "component"),
        record.definition.name.as_str(),
        counted(record.fields.len(), "field"),
    ))
    .with_source_span(at)
}

fn no_positional_decomposition(
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
    reason: NoRecord<'_>,
    at: SourceSpan,
) -> Diagnostic {
    let subject = shown(expressions, subject);
    Diagnostic::error(match reason {
        NoRecord::Sum(sum) => format!(
            "a positional pattern decomposes a record into its fields, and `{subject}` is the \
             sum `{}`: match its cases instead",
            sum.name.as_str()
        ),
        NoRecord::NotData => format!(
            "a positional pattern decomposes a record into its fields, and `{subject}` is not a \
             record"
        ),
        NoRecord::UnknownType => format!(
            "a positional pattern selects fields by their place in the subject's record, and \
             the record of `{subject}` is not declared here: give `{subject}` a declared record \
             type"
        ),
    })
    .with_source_span(at)
}

fn no_rest(
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
    reason: NoRecord<'_>,
    at: SourceSpan,
) -> Diagnostic {
    let subject = shown(expressions, subject);
    Diagnostic::error(match reason {
        NoRecord::Sum(sum) => format!(
            "`..` waives the remaining fields of a record, and `{subject}` is the sum `{}`: \
             match its cases instead",
            sum.name.as_str()
        ),
        NoRecord::NotData => {
            format!("`..` waives the remaining fields of a record, and `{subject}` is not a record")
        }
        NoRecord::UnknownType => format!(
            "`..` waives the fields this pattern does not name, and the record of `{subject}` \
             is not declared here: give `{subject}` a declared record type"
        ),
    })
    .with_source_span(at)
}

/// `let Pair { first, second } = subject;` names the record it decomposes.
/// The name has to be the subject's own record: a pattern that named another
/// type would bind fields the subject does not have.
///
/// A subject whose record is not declared here passes: the lowered-form
/// reader still compares the marker's record with the value it opens.
pub(crate) fn check_head<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    subject: ExpressionHandle,
    head: &resolved::types::TypeReferenceNode,
) -> Result<Record<'program>, Diagnostic> {
    use resolved::types::TypeReferenceNode;
    let (symbol, name, at) = match head {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str(), name.source_span()),
        TypeReferenceNode::Generic { base_name, .. } => {
            return Err(Diagnostic::error(format!(
                "a data pattern cannot name `{}` with type arguments yet: decompose the value \
                 with the headless `{{ ... }}` pattern",
                base_name.as_str()
            ))
            .with_source_span(base_name.source_span()));
        }
        _ => {
            return Err(Diagnostic::error(
                "a data pattern names the record it decomposes, and this is not a record's name",
            )
            .with_source_span(expressions.source_span(subject)));
        }
    };
    let refused = |message: String| Err(Diagnostic::error(message).with_source_span(at));
    if symbol.is_valid() && program.symbols.get(symbol).kind == symbols::SymbolKind::Variant {
        // patterns.md does not say whether a `let` may name a case that the
        // subject's facts prove. A one-arm `match` expresses that program,
        // so the pattern is refused instead of guessed at.
        return refused(format!(
            "a `let` pattern names the record it decomposes, and `{name}` is a case: match \
             the subject by case instead"
        ));
    }
    let definition = program
        .data_definitions
        .iter()
        .find(|definition| symbol.is_valid() && definition.symbol == symbol);
    let named = match definition
        .ok_or(NoRecord::NotData)
        .and_then(|definition| defined_record(program, definition))
    {
        Ok(record) => record,
        Err(NoRecord::Sum(sum)) => {
            return refused(format!(
                "a `let` pattern decomposes a record, and `{}` is a sum: match its cases instead",
                sum.name.as_str()
            ));
        }
        Err(NoRecord::NotData | NoRecord::UnknownType) => {
            return refused(format!(
                "`{name}` does not name a record: a `let` pattern names the record it \
                 decomposes, and a case is matched in a `match` or `transition` arm"
            ));
        }
    };
    let shown = shown(expressions, subject);
    match subject_record(program, expressions, subject) {
        Ok(actual) if actual.definition.symbol == named.definition.symbol => Ok(named),
        Ok(actual) => refused(format!(
            "this pattern names `{name}`, but `{shown}` is a `{}`",
            actual.definition.name.as_str()
        )),
        Err(NoRecord::Sum(sum)) => refused(format!(
            "this pattern names the record `{name}`, but `{shown}` is the sum `{}`: match its \
             cases instead",
            sum.name.as_str()
        )),
        Err(NoRecord::NotData) => refused(format!(
            "this pattern names the record `{name}`, but `{shown}` is not a record"
        )),
        Err(NoRecord::UnknownType) => Ok(named),
    }
}
