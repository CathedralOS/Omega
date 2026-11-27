//! Check named constants against their actual source invocations and canonical values.
#[path = "constants/arguments.rs"]
mod arguments;
#[path = "constants/initializers.rs"]
mod initializers;
pub(super) use arguments::Arguments;
pub(super) use initializers::{Initializers, retain_evaluation_entries};

use crate::source_state::TypedTrees;
use crate::source_state::data::DataField;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::references::{DeclarationSelection, TypedReferences};
use application_identity::const_value::DecodedCanonicalConstValue;
use symbols::SymbolHandle;

use build_model::values::BuildTimeValue;
use source_completion_accounting::{
    PureEvaluationRecord, PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};

pub(super) fn key(symbol: SymbolHandle) -> (u32, u64) {
    (symbol.arena_index(), u64::from(symbol.generation()))
}

// Sorted source-family rows preserve the full arena handle, including its
// generation. Scratch below is sized to those rows, never a global symbol ID.
fn family_position(family: &[SymbolHandle], symbol: SymbolHandle) -> Option<usize> {
    family
        .binary_search_by_key(&key(symbol), |symbol| key(*symbol))
        .ok()
}
pub(super) fn insert(family: &mut Vec<SymbolHandle>, symbol: SymbolHandle) -> bool {
    match family.binary_search_by_key(&key(symbol), |symbol| key(*symbol)) {
        Ok(_) => false,
        Err(index) => {
            family.insert(index, symbol);
            true
        }
    }
}
fn insert_expression(family: &mut Vec<ExpressionHandle>, expression: ExpressionHandle) -> bool {
    let key = |expression: ExpressionHandle| (expression.arena_index(), expression.generation());
    match family.binary_search_by_key(&key(expression), |expression| key(*expression)) {
        Ok(_) => false,
        Err(index) => {
            family.insert(index, expression);
            true
        }
    }
}

/// The declarations each evaluated constant's computation reached, sorted by
/// constant. The demand order computed each row once, before any value ran,
/// and made the constant wait for every value those declarations hold.
#[derive(Debug, Default)]
pub struct ConstantReaches {
    rows: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
}

impl ConstantReaches {
    pub(super) fn new(rows: impl IntoIterator<Item = (SymbolHandle, Vec<SymbolHandle>)>) -> Self {
        let mut rows = rows.into_iter().collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(constant, _)| key(*constant));
        Self { rows }
    }

    fn of(&self, constant: SymbolHandle) -> Option<&[SymbolHandle]> {
        self.rows
            .binary_search_by_key(&key(constant), |(constant, _)| key(*constant))
            .ok()
            .map(|index| self.rows[index].1.as_slice())
    }
}

/// Requires every declaration in `reached` to read the same in `program` as
/// in `source`, the program the constant was evaluated in: each type written
/// in it, the selected storage layouts its computation used, its access plan
/// as a domain, and its value as a constant.
///
/// The demand order evaluated the constant only after every value those
/// declarations hold. If it missed one, a later round changed what the
/// constant read and this refuses. Declarations outside `reached` may differ:
/// later rounds settle their own values.
fn reached_declarations_unchanged(
    source: &TypedTrees,
    program: &TypedTrees,
    reached: &[SymbolHandle],
) -> Result<(), &'static str> {
    let is_reached = |symbol: SymbolHandle| family_position(reached, symbol).is_some();
    const TYPE: &str = "constant receipt read a type that changed after the constant was evaluated";
    const LAYOUT: &str =
        "constant receipt read a layout that changed after the constant was evaluated";
    let selection = DeclarationSelection::from_symbols(reached.iter().copied());
    let references =
        crate::source_state::references::collect_selected_program_references(program, &selection);
    for &reference in &references.type_references {
        if !source
            .type_reference_table
            .contains_type_reference(reference)
            || source.type_reference_table.type_reference(reference)
                != program.type_reference_table.type_reference(reference)
        {
            return Err(TYPE);
        }
    }

    // A semantic data declaration can have several independently selected
    // storage applications. Publication retains all those plans with the data.
    // The constant observes only the applications used by its computation.
    if !source.plan_laid_layouts.is_empty() || !program.plan_laid_layouts.is_empty() {
        let inputs =
            crate::source_state::references::selected_storage_type_references(program, &selection);
        for reference in inputs {
            if !source
                .type_reference_table
                .contains_type_reference(reference)
            {
                return Err(TYPE);
            }
            match (
                source.plan_laid_layout_for_type(reference),
                program.plan_laid_layout_for_type(reference),
            ) {
                (None, None) => {}
                (Some(before), Some(after))
                    if before == after
                        && before.target_layouts(source) == after.target_layouts(program) => {}
                _ => return Err(LAYOUT),
            }
        }
    }
    for (_, domain) in program.domain_definitions.iter() {
        if !is_reached(domain.symbol) {
            continue;
        }
        let earlier = source
            .domain_definitions
            .iter()
            .find(|(_, earlier)| earlier.symbol == domain.symbol)
            .map(|(_, earlier)| &earlier.access_policy);
        if earlier != Some(&domain.access_policy) {
            return Err(
                "constant receipt read an access plan that changed after the constant was \
                 evaluated",
            );
        }
    }
    for constant in program.const_declarations() {
        if is_reached(constant.symbol)
            && source.const_declaration_by_symbol(constant.symbol) != Some(constant)
        {
            return Err(
                "constant receipt read a constant that changed after the constant was evaluated",
            );
        }
    }
    Ok(())
}

/// Check each computed constant against the invocation that evaluated it.
/// The requester, source program and returned value must agree.
pub fn validate_constant_evaluations(
    program: &TypedTrees,
    accounting: &SourceCompletionAccounting,
    reaches: &ConstantReaches,
) -> Result<(), String> {
    accounting.validate_charges()?;
    let mut wanted = program
        .const_declarations()
        .iter()
        .map(|constant| constant.symbol)
        .collect::<Vec<_>>();
    wanted.sort_unstable_by_key(|symbol| key(*symbol));
    for symbol in &wanted {
        if !program.symbols.symbols().nodes().is_valid(*symbol) {
            return Err("constant source has a stale declaration".into());
        }
    }
    // A target-independent constant has one receipt. One computed through an
    // authored layout has one receipt per deployment that reads it.
    let mut receipts = vec![Vec::new(); wanted.len()];
    // A layout policy run for a constant's own storage type also names the
    // constant as its requester, but runs a policy plan rather than a machine
    // generated from a constant, so it supplies no constant value.
    let initializer = |machine: SymbolHandle| {
        program.symbols.symbols().nodes().is_valid(machine)
            && family_position(&wanted, program.symbols.get(machine).generated_from).is_some()
    };
    for record in accounting.receipts() {
        let PureEvaluationRecord::Invocation(receipt) = record else {
            continue;
        };
        if let Some(source) = accounting.constant_input(receipt)? {
            if program.identity.get() != source.identity.get() {
                return Err("constant receipt belongs to another source owner".into());
            }
            if family_position(&wanted, receipt.requester()).is_none() {
                return Err("constant receipt lost its requesting declaration".into());
            }
        }
        if let Some(index) = family_position(&wanted, receipt.requester())
            && (initializer(receipt.machine())
                || program
                    .const_declaration_by_symbol(receipt.requester())
                    .is_some_and(|constant| {
                        !program.tables.machines.is_valid(constant.evaluation_entry)
                    }))
        {
            let target = receipt.admission().target();
            let rows: &mut Vec<&PureTerminalEvaluationReceipt> = &mut receipts[index];
            if rows.iter().any(|row| {
                let other = row.admission().target();
                other.is_none() || target.is_none() || other == target
            }) {
                return Err("constant result has ambiguous invocation custody".into());
            }
            rows.push(receipt);
        }
    }
    let mut computations = vec![false; wanted.len()];
    if !wanted.is_empty() {
        for machine in program.machines() {
            if !program.symbols.symbols().nodes().is_valid(machine.symbol) {
                return Err("constant source has an invalid machine declaration".into());
            }
            if let Some(index) =
                family_position(&wanted, program.symbols.get(machine.symbol).generated_from)
            {
                computations[index] = true;
            }
        }
    }
    if !computations.iter().any(|present| *present)
        && receipts.iter().all(Vec::is_empty)
        && program.const_declarations().iter().all(|constant| {
            !constant.evaluation_entry.is_valid() && !constant.evaluation_expression.is_valid()
        })
    {
        return Ok(());
    }
    // A constant invocation may add expressions to materialize its result.
    // All expressions from before the first invocation must stay intact.
    let mut original_source = None;
    for record in accounting.receipts() {
        if let PureEvaluationRecord::Invocation(receipt) = record
            && family_position(&wanted, receipt.requester()).is_some()
            && let Some(source) = accounting.constant_input(receipt)?
        {
            original_source = Some(source);
            break;
        }
    }
    if let Some(source) = original_source
        && !program
            .expression_table
            .retains_input_storage(&source.expression_table)
    {
        return Err("constant receipt has changed expression input storage".into());
    }
    for constant in program.const_declarations() {
        let index =
            family_position(&wanted, constant.symbol).ok_or("constant declaration family drift")?;
        if !constant.evaluation_entry.is_valid() {
            if constant.evaluation_expression.is_valid()
                || computations[index]
                || !receipts[index].is_empty()
            {
                return Err("constant result lost its initializer computation custody".into());
            }
            continue;
        }
        if !program.tables.machines.is_valid(constant.evaluation_entry) {
            return Err("constant result has a stale initializer entry".into());
        }
        let entry = program.tables.machines.get(constant.evaluation_entry);
        // An unclaimed request remains pending. It has no value to justify,
        // and must not force evaluation merely because the source retained it.
        if constant.evaluated_value.is_none() {
            if !receipts[index].is_empty() {
                return Err("constant invocation receipt lost its materialized result".into());
            }
            continue;
        }
        if receipts[index].is_empty() {
            return Err("constant result has no accepted invocation receipt".into());
        }
        for receipt in &receipts[index] {
            if receipt.machine() != entry.symbol {
                return Err("constant result has swapped invocation custody".into());
            }
            let source = accounting
                .constant_input(receipt)?
                .ok_or("constant receipt lost its original requesting declaration")?
                .as_ref();
            if program.identity.get() != source.identity.get() {
                return Err("constant receipt belongs to another source owner".into());
            }
            // The receipt's source is the program of the round that evaluated
            // this constant (`Initializers` ties the two when the receipt is
            // recorded). Values that settled in later rounds changed their own
            // types, layouts and plans since, so the two programs differ
            // outside what this constant read. Inside it they must not: the
            // declarations its computation reached, its own declaration, and
            // every expression its initializer reaches are compared exactly.
            let reached = reaches
                .of(constant.symbol)
                .ok_or("constant receipt has no recorded reach")?;
            reached_declarations_unchanged(source, program, reached)?;
            let original = source
                .const_declaration_by_symbol(constant.symbol)
                .ok_or("constant receipt lost its original requesting declaration")?;
            let mut retained = original.clone();
            retained.materialized_initializer = constant.materialized_initializer;
            retained.evaluated_value = constant.evaluated_value.clone();
            if &retained != constant {
                return Err("constant receipt belongs to another initializer request".into());
            }
            if !program
                .expression_table
                .expression_is_valid(constant.authored_initializer)
                || !program
                    .expression_table
                    .expression_is_valid(constant.materialized_initializer)
            {
                return Err("constant lost its authored or materialized expression".into());
            }
            let references = request_references(program, constant.authored_initializer);
            for expression in references.expressions {
                if !source.expression_table.expression_is_valid(expression)
                    || source.expression_table.expression(expression)
                        != program.expression_table.expression(expression)
                    || source.expression_table.source_span(expression)
                        != program.expression_table.source_span(expression)
                {
                    return Err("constant receipt has a stale authored computation".into());
                }
            }
            let source_completion_accounting::PureTerminalEvaluationOutcome::Materialized {
                value,
                ..
            } = receipt.outcome()
            else {
                return Err("constant receipt did not return a materialized value".into());
            };
            let value = decoded(program, constant.declared_type, value)?;
            let expected =
                application_identity::const_value::CanonicalConstIdentity::from_declaration_value(
                    &value,
                )
                .ok_or("constant receipt result has no canonical encoding")?;
            if constant
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
                != Some(expected.encoding.as_str())
            {
                return Err("constant value differs from its accepted invocation outcome".into());
            }
            if !program.canonical_const_materialization_matches(
                constant.declared_type,
                constant.materialized_initializer,
                &value,
            ) {
                return Err("constant materialization differs from its invocation outcome".into());
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct References {
    symbols: Vec<SymbolHandle>,
    calls: Vec<SymbolHandle>,
    expressions: Vec<ExpressionHandle>,
}
impl Extend<SymbolHandle> for References {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            insert(&mut self.symbols, symbol);
        }
    }
}
impl TypedReferences for References {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        self.calls.push(symbol);
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        insert_expression(&mut self.expressions, expression)
    }
}

fn request_references(program: &TypedTrees, expression: ExpressionHandle) -> References {
    let mut references = References::default();
    crate::source_state::references::visit_expression(program, &mut references, expression);
    references.follow_calls(program)
}

impl References {
    fn follow_calls(mut self, program: &TypedTrees) -> Self {
        let mut visited = Vec::new();
        while let Some(callee) = self.calls.pop() {
            let Some(machine) = program
                .machine_by_symbol(callee)
                .or_else(|| program.machine_holding_state(callee))
            else {
                continue;
            };
            if insert(&mut visited, machine.symbol) {
                crate::source_state::references::visit_machine(program, &mut self, machine);
            }
        }
        self
    }
}

/// Decode the evaluator's snapshot against the constant's exact carrier.
/// References and authority-bearing values have no constant representation.
pub(super) fn decoded(
    program: &TypedTrees,
    carrier: crate::source_state::types::TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    if !program
        .type_reference_table
        .contains_type_reference(carrier)
        || program.stored_value_multiplicity(carrier) != language_core::Multiplicity::Unrestricted
    {
        return Err("constant result does not have an unrestricted carrier".into());
    }
    decode_snapshot(program, carrier, value)
}

fn decode_snapshot(
    program: &TypedTrees,
    carrier: crate::source_state::types::TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    use crate::source_state::types::{PrimitiveType, TypeReferenceNode};
    let unsupported =
        || "no product route yet: constant result needs its pure snapshot materializer".to_owned();
    // The snapshot keeps the value, not the evidence the invocation checked
    // for each domain on it, so materialization re-establishes each domain
    // with `as +D`. Anyone may write that only for a domain with
    // unrestricted issuance; a restricted domain needs its issuer, and a
    // constant grants no authority (constants.md).
    for domain in program
        .constant_domain_qualifications(carrier)
        .ok_or_else(unsupported)?
    {
        let definition = program
            .domain_definition(domain.symbol)
            .ok_or("constant result names a domain without its declaration")?;
        if definition.issuance() != crate::source_state::domain::DomainIssuance::Unrestricted {
            return Err(format!(
                "a constant grants no authority, and `{}` restricts who may issue it: \
                 the constant would re-establish it from the evaluated value; \
                 declare the constant with the bare carrier and qualify it through an issuer",
                definition.name.as_str(),
            ));
        }
    }
    let underlying = program
        .unwrapped_type_reference(carrier)
        .ok_or("constant result has no exact carrier")?;
    // `LayoutStorage<T, Policy>` holds T's fields at the selected plan's
    // offsets (layouts/plans.md, "Representation applications"). Its
    // snapshot is T's value; the layout only places those fields.
    if let Some(storage) = program.layout_storage_arguments(underlying) {
        let BuildTimeValue::Struct { type_name, fields } = value else {
            return Err("constant result is not a record of its declared layout storage".into());
        };
        if *type_name != program.normalized_type_identity_text(underlying) {
            return Err("constant result differs from its declared layout storage".into());
        }
        let declaration = program
            .closed_data_definition(storage.semantic_type)
            .ok_or("constant layout storage lost its closed semantic record")?;
        let fields = decode_snapshot_fields(
            program,
            program
                .data_members(declaration)
                .iter()
                .map(|member| member.as_field()),
            fields,
        )?;
        return Ok(DecodedCanonicalConstValue::Record {
            type_name: declaration.name.as_str().to_owned(),
            fields,
        });
    }
    let symbol = match program.type_reference_table.type_reference(underlying) {
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("constant result is not an array of its declared type".into());
            };
            let length = program.closed_fixed_array_length(underlying).ok_or("array type has no evaluated extent")?;
            if elements.len() != length {
                return Err("constant result differs from its declared array extent".into());
            }
            let values = elements
                .iter()
                .map(|element| decode_snapshot(program, *element_type, element))
                .collect::<Result<_, _>>()?;
            return Ok(DecodedCanonicalConstValue::Array {
                type_name: program.display_type_reference(carrier),
                values,
            });
        }
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        _ => return Err(unsupported()),
    };
    if let Some(declaration) = program.data_definition_by_symbol(symbol) {
        let members = program.data_members(declaration);
        let declared_name = declaration.name.as_str().to_owned();
        return match value {
            BuildTimeValue::Struct { type_name, fields } => {
                if *type_name != program.normalized_type_identity_text(underlying) {
                    return Err("constant result differs from its declared record type".into());
                }
                let fields = decode_snapshot_fields(
                    program,
                    members.iter().map(|member| member.as_field()),
                    fields,
                )?;
                Ok(DecodedCanonicalConstValue::Record {
                    type_name: declared_name,
                    fields,
                })
            }
            BuildTimeValue::Case { variant, payload } => {
                // The invocation already checked the result carrier. Resolve
                // its observed case only within that exact declaration.
                let mut matching = members
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .filter(|case| case.member_identity().spelling() == *variant);
                let (Some(case), None) = (matching.next(), matching.next()) else {
                    return Err("constant result has no exact case of its declared type".into());
                };
                let fields = decode_snapshot_fields(
                    program,
                    program.data_payload_fields(case).iter().map(Some),
                    payload,
                )?;
                Ok(DecodedCanonicalConstValue::Variant {
                    type_name: declared_name,
                    case_name: case.name.as_str().to_owned(),
                    fields,
                })
            }
            _ => Err(unsupported()),
        };
    }
    let atom = program
        .symbols
        .builtin_type_atom(symbol)
        .ok_or_else(unsupported)?;
    let primitive = match atom {
        symbols::BuiltinTypeAtom::Bool => PrimitiveType::Bool,
        atom => PrimitiveType::from_builtin_atom(atom)
            .ok_or("constant result has no builtin carrier")?,
    };
    match (primitive, value) {
        (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
            Ok(DecodedCanonicalConstValue::Boolean(*value))
        }
        (
            PrimitiveType::F32,
            BuildTimeValue::Float(semantic_vocabulary::IeeeFloatValue::Binary32(bits)),
        ) => Ok(DecodedCanonicalConstValue::Float {
            format: numeric_literals::FloatFormat::F32,
            bits: u64::from(*bits),
        }),
        (
            PrimitiveType::F64,
            BuildTimeValue::Float(semantic_vocabulary::IeeeFloatValue::Binary64(bits)),
        ) => Ok(DecodedCanonicalConstValue::Float {
            format: numeric_literals::FloatFormat::F64,
            bits: *bits,
        }),
        (_, BuildTimeValue::UndeterminedNaN(_)) => {
            Err("a floating constant with undetermined NaN bits cannot be materialized".into())
        }
        (primitive, BuildTimeValue::Int(value)) if primitive.accepts_integer_literal() => {
            Ok(DecodedCanonicalConstValue::Integer {
                integer_type: primitive
                    .landed_landing()
                    .ok_or("constant result has no integer carrier")?,
                // BuildTimeValue::Int retains the complete u64 bit pattern.
                value: if primitive.is_signed_integer() {
                    bignum::BigInt::from(*value)
                } else {
                    bignum::BigInt::from(*value as u64)
                },
            })
        }
        _ => Err(unsupported()),
    }
}

fn decode_snapshot_fields<'a>(
    program: &TypedTrees,
    declared: impl ExactSizeIterator<Item = Option<&'a DataField>>,
    values: &[(String, BuildTimeValue)],
) -> Result<Vec<(String, DecodedCanonicalConstValue)>, String> {
    if declared.len() != values.len() {
        return Err("constant result differs from its declared field count".into());
    }
    declared
        .zip(values)
        .map(|(field, (identity, value))| {
            let field = field.ok_or("constant result does not match its declared record shape")?;
            if field.member_identity().spelling() != *identity {
                return Err("constant result differs from its declared field identity".into());
            }
            Ok((
                field.name.as_str().to_owned(),
                decode_snapshot(program, field.type_reference, value)?,
            ))
        })
        .collect()
}

#[cfg(test)]
#[path = "constants/tests.rs"]
mod tests;
