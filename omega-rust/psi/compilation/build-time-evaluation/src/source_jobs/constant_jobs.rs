//! Evaluates retained constant computations through the accepted invocation
//! bridge, then stores their snapshots in their original typed owner.
//! Dependency discovery reads identities only; it never interprets an
//! expression or chooses a result. Every job still passes typed preparation,
//! structural discharge, publication and hermetic invocation admission.

use std::collections::VecDeque;

use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataField;
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::TypedReferences;

use crate::{BuildTimeValue, SourceCompletionAccounting};

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

/// Join the emitted value to its actual accepted invocation before another
/// source owner is frozen. Merely retaining an initializer machine cannot
/// justify a substituted literal: the requester, source, call and outcome
/// must still be the ones the interpreter used.
pub fn validate_constant_evaluations(
    program: &TypedTrees,
    accounting: &SourceCompletionAccounting,
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
        let crate::PureEvaluationRecord::Invocation(receipt) = record else {
            continue;
        };
        if receipt
            .request_source()
            .const_declaration_by_symbol(receipt.requester())
            .is_some()
        {
            if program.identity.get() != receipt.request_source().identity.get() {
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
            let rows: &mut Vec<&crate::PureTerminalEvaluationReceipt> = &mut receipts[index];
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
    // The first actual constant invocation still owns every original named
    // use, before this batch substitutes its first value. Prepare that use
    // family once and authenticate each emitted copy against its own receipt.
    let original_source = accounting
        .receipts()
        .iter()
        .find_map(|record| match record {
            crate::PureEvaluationRecord::Invocation(receipt)
                if family_position(&wanted, receipt.requester()).is_some() =>
            {
                Some(receipt.request_source())
            }
            _ => None,
        });
    if let Some(source) = original_source
        && !program
            .expression_table
            .retains_input_storage(&source.expression_table)
    {
        return Err("constant receipt has changed expression input storage".into());
    }
    let original_uses = original_source.map(|source| named_uses(source, &wanted));
    // Reuse one comparison program for all constants so its owned tables are
    // copied once for this validation.
    let mut comparison = program.clone();
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
            let source = receipt.request_source();
            if program.identity.get() != source.identity.get() {
                return Err("constant receipt belongs to another source owner".into());
            }
            // Evaluation changes the constant records and materialized expression
            // values. All other source data must be the original request owner.
            // The expression dependency closure is compared separately below, so
            // later independent constant materializations need not be identical.
            comparison.expression_table = source.expression_table.clone();
            comparison.tables.const_declarations = source.tables.const_declarations.clone();
            if &comparison != source {
                return Err("constant receipt has stale declaration or execution metadata".into());
            }
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
            let crate::PureTerminalEvaluationOutcome::Materialized { value, .. } =
                receipt.outcome()
            else {
                return Err("constant receipt did not return a materialized value".into());
            };
            let value = decoded(program, constant.declared_type, value)?;
            let expected =
                language_semantics::const_value::CanonicalConstIdentity::from_declaration_value(
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
            let uses = original_uses
                .as_ref()
                .map(|rows| rows[index].as_slice())
                .unwrap_or_default();
            for expression in
                std::iter::once(constant.materialized_initializer).chain(uses.iter().copied())
            {
                if !program.canonical_const_materialization_matches(
                    constant.declared_type,
                    expression,
                    &value,
                ) {
                    return Err(
                        "constant materialization differs from its invocation outcome".into(),
                    );
                }
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

fn dependencies(program: &TypedTrees, entry: SymbolHandle) -> Vec<SymbolHandle> {
    let mut references = References::default();
    let mut pending = vec![entry];
    let mut visited = Vec::new();
    while let Some(callee) = pending.pop() {
        let Some(machine) = program
            .machine_by_symbol(callee)
            .or_else(|| program.machine_holding_state(callee))
        else {
            continue;
        };
        if !insert(&mut visited, machine.symbol) {
            continue;
        }
        typed_trees::references::visit_machine(program, &mut references, machine);
        pending.append(&mut references.calls);
    }
    references.symbols
}

fn request_references(program: &TypedTrees, expression: ExpressionHandle) -> References {
    let mut references = References::default();
    typed_trees::references::visit_expression(program, &mut references, expression);
    let mut visited = Vec::new();
    while let Some(callee) = references.calls.pop() {
        let Some(machine) = program
            .machine_by_symbol(callee)
            .or_else(|| program.machine_holding_state(callee))
        else {
            continue;
        };
        if insert(&mut visited, machine.symbol) {
            typed_trees::references::visit_machine(program, &mut references, machine);
        }
    }
    references
}

/// Decode the evaluator's snapshot against the constant's exact carrier.
/// References and authority-bearing values have no constant representation.
pub(super) fn decoded(
    program: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    if !program
        .type_reference_table
        .contains_type_reference(carrier)
        || program.stored_value_multiplicity(carrier)
            != language_semantics::Multiplicity::Unrestricted
    {
        return Err("constant result does not have an unrestricted carrier".into());
    }
    decode_snapshot(program, carrier, value)
}

fn decode_snapshot(
    program: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<DecodedCanonicalConstValue, String> {
    use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceNode};
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
        if definition.issuance() != typed_trees::domain::DomainIssuance::Unrestricted {
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
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("constant result is not an array of its declared type".into());
            };
            if elements.len() != *length {
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
        (primitive, BuildTimeValue::Int(value)) if primitive.accepts_integer_literal() => {
            Ok(DecodedCanonicalConstValue::Integer {
                integer_type: primitive
                    .landed_landing()
                    .ok_or("constant result has no integer carrier")?,
                // BuildTimeValue::Int retains the complete u64 bit pattern.
                value: if primitive.is_signed_integer() {
                    numerics::bignum::BigInt::from(*value)
                } else {
                    numerics::bignum::BigInt::from(*value as u64)
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

fn named_uses(program: &TypedTrees, constants: &[SymbolHandle]) -> Vec<Vec<ExpressionHandle>> {
    // Authored initializers remain source correspondence. Visit ordinary
    // declarations and emitted initializer machines, not those originals.
    struct Uses(References);
    impl Extend<SymbolHandle> for Uses {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for Uses {
        fn derived_declaration_references(&self) -> bool {
            false
        }
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.0.symbols.contains(&symbol)
        }
        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            insert_expression(&mut self.0.expressions, expression)
        }
    }
    let mut uses = Uses(References {
        symbols: program
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect(),
        ..Default::default()
    });
    typed_trees::references::visit_program(program, &mut uses);
    let mut named = vec![Vec::new(); constants.len()];
    for expression in uses.0.expressions {
        if let Some(symbol) = program
            .expression_table
            .name_path_node(expression)
            .and_then(|path| path.resolved_symbol())
            && let Some(index) = family_position(constants, symbol)
        {
            named[index].push(expression);
        }
    }
    named
}

fn materialize_uses(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    expressions: &[ExpressionHandle],
) -> Result<(), String> {
    let declaration = program
        .const_declaration_by_symbol(symbol)
        .ok_or("constant use lost its exact declaration")?;
    let carrier = declaration.declared_type;
    let value = typed_trees::canonical_const_declaration_value(program, symbol)
        .and_then(|value| value.decode_encoding())
        .ok_or("constant use has no evaluated canonical value")?;
    for expression in expressions {
        let node = program
            .materialize_canonical_const_value(carrier, &value)
            .ok_or("constant value does not materialize in its declared carrier")?;
        *program.expression_table.expression_mut(*expression) = node;
    }
    Ok(())
}

/// Nominate computation from the same request the lowered-form checker will
/// judge. This discovers identities only; neither membership nor an empty
/// dependency row admits an invocation or supplies its result.
fn demanded_constants(
    program: &TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
) -> Result<Vec<SymbolHandle>, String> {
    let selected = demanded_declarations(program, request)?;
    let mut constants = program
        .const_declarations()
        .iter()
        .filter(|constant| selected.contains(constant.symbol))
        .map(|constant| constant.symbol)
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|symbol| key(*symbol));
    constants.dedup();
    Ok(constants)
}

/// Every declaration the request reaches from its entries, selections,
/// bindings, subjects and supplies through the declarations each one names,
/// sorted. A const argument is demanded when one of these declarations
/// holds it (`const_argument_jobs`).
pub(super) fn demanded_declarations(
    program: &TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
) -> Result<typed_trees::references::DeclarationSelection, String> {
    let mut pending = request
        .entries
        .iter()
        .chain(request.selected)
        .copied()
        .collect::<Vec<_>>();
    for binding in request.bindings {
        pending.extend([
            binding.implementation,
            binding.semantic_schema_symbol,
            binding.requirement_symbol,
        ]);
    }
    if !request.subjects.is_empty() {
        for symbol in typed_trees::references::collect_program_declarations(program) {
            if typed_trees::source_subjects::contains(
                program,
                symbol,
                request.subjects,
                request.purpose,
            )? {
                pending.push(symbol);
            }
        }
    }
    // Supply bodies are roots of this request too. Match their genuine typed
    // conformance rows; structural judgment still checks exact slot coverage
    // and refinement, so this nomination cannot create a supplied proof.
    for supply in request.supplies {
        pending.extend([supply.slot, supply.provider]);
        let requirements = if let Some(owner) = program.trait_definition_by_symbol(supply.slot) {
            program
                .trait_machine_signatures(owner)
                .iter()
                .map(|row| row.symbol)
                .collect::<Vec<_>>()
        } else {
            vec![supply.slot]
        };
        for machine in program.machines() {
            if typed_trees::provider_declarations::satisfaction_provider_declaration(
                program, machine,
            ) == Some(supply.provider)
                && (program
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|row| requirements.contains(&row.requirement_symbol))
                    || program
                        .owned_conformance_realization(machine.symbol)
                        .is_some_and(|(_, row)| requirements.contains(&row.requirement)))
            {
                pending.push(machine.symbol);
            }
        }
    }
    typed_trees::references::DeclarationSelection::from_computation_roots(program, pending)
        .map_err(str::to_owned)
}

pub fn evaluate(
    program: &mut TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    evaluate_with_source_consent(program, request, &[], authority, None, accounting)
}

/// The deployments whose declaration closure reaches each constant, in
/// target order. A constant computed through an authored storage layout
/// runs once under each of them; others ignore this.
fn deployment_targets<'scope>(
    program: &TypedTrees,
    scopes: &'scope [super::FixedLayoutTargetScope],
    constants: &[SymbolHandle],
) -> Result<Vec<Vec<&'scope super::FixedLayoutTargetScope>>, String> {
    let mut reached = vec![Vec::new(); constants.len()];
    if constants.is_empty() {
        return Ok(reached);
    }
    let mut ordered = scopes
        .iter()
        .filter(|scope| {
            !scope.entry_declarations.is_empty() || !scope.provider_declarations.is_empty()
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.target.cmp(&right.target));
    for scope in ordered {
        let selection = typed_trees::references::DeclarationSelection::from_computation_roots(
            program,
            scope.entry_declarations.iter().copied().chain(
                scope
                    .provider_declarations
                    .iter()
                    .map(|row| row.provider_type.symbol),
            ),
        )?;
        for (position, constant) in constants.iter().enumerate() {
            let targets: &mut Vec<&super::FixedLayoutTargetScope> = &mut reached[position];
            if !selection.contains(*constant) {
                continue;
            }
            match targets.last() {
                Some(last) if last.target == scope.target => {
                    if last.facts != scope.facts {
                        return Err("layout scopes disagree on one target's immutable facts".into());
                    }
                }
                _ => targets.push(scope),
            }
        }
    }
    Ok(reached)
}

pub fn evaluate_with_source_consent(
    program: &mut TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
    scopes: &[super::FixedLayoutTargetScope],
    authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    source_consent: Option<std::sync::Arc<dyn crate::InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let demanded =
        demanded_constants(program, request).map_err(|reason| vec![Diagnostic::error(reason)])?;
    let jobs = program
        .roots
        .const_declarations
        .handles()
        .filter_map(|handle| {
            let constant = program.tables.const_declarations.get(handle);
            (family_position(&demanded, constant.symbol).is_some()
                && constant.evaluated_value.is_none()
                && program.tables.machines.is_valid(constant.evaluation_entry))
            .then(|| {
                (
                    handle,
                    constant.symbol,
                    program
                        .tables
                        .machines
                        .get(constant.evaluation_entry)
                        .symbol,
                )
            })
        })
        .collect::<Vec<_>>();
    if jobs.is_empty() {
        return Ok(demanded);
    }
    let mut constants = program
        .const_declarations()
        .iter()
        .map(|constant| constant.symbol)
        .collect::<Vec<_>>();
    constants.sort_unstable_by_key(|symbol| key(*symbol));
    let uses = named_uses(program, &constants);
    for (symbol, expressions) in constants.iter().zip(&uses) {
        if program
            .const_declaration_by_symbol(*symbol)
            .is_some_and(|constant| constant.evaluated_value.is_some())
        {
            materialize_uses(program, *symbol, expressions)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
    }
    let mut positions = jobs
        .iter()
        .enumerate()
        .map(|(index, (_, symbol, _))| (*symbol, index))
        .collect::<Vec<_>>();
    positions.sort_unstable_by_key(|(symbol, _)| key(*symbol));
    let mut successors = vec![Vec::new(); jobs.len()];
    let mut remaining = vec![0usize; jobs.len()];
    for (position, (_, _, entry)) in jobs.iter().enumerate() {
        for dependency in dependencies(program, *entry) {
            if let Ok(index) =
                positions.binary_search_by_key(&key(dependency), |(symbol, _)| key(*symbol))
            {
                let dependency = positions[index].1;
                remaining[position] += 1;
                successors[dependency].push(position);
            }
        }
    }
    let job_symbols = jobs
        .iter()
        .map(|(_, symbol, _)| *symbol)
        .collect::<Vec<_>>();
    let deployments = deployment_targets(program, scopes, &job_symbols)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut ready = remaining
        .iter()
        .enumerate()
        .filter_map(|(position, count)| (*count == 0).then_some(position))
        .collect::<VecDeque<_>>();
    let mut completed = 0usize;
    while let Some(position) = ready.pop_front() {
        let (handle, symbol, entry) = jobs[position];
        let result = (|| -> Result<(), Vec<Diagnostic>> {
            let prepared = super::PreparedBuildMachineProgram::prepare_for(
                program,
                authority.clone(),
                request.purpose,
                source_consent.clone(),
            )?;
            let targets = deployments[position]
                .iter()
                .map(|scope| (scope.target.as_str(), scope.facts.as_ref()))
                .collect::<Vec<_>>();
            let invocation = prepared.prepare_entry(symbol, entry, &[])?;
            let module = invocation.accepted().module();
            if module
                .scalar_applications
                .declarations()
                .iter()
                .any(|application| {
                    matches!(
                        application,
                        terminal_psi::ScalarApplication::Evaluate { .. }
                    )
                })
                || module.boundary_machines.iter().any(|boundary| {
                    boundary
                        .declaration
                        .as_ref()
                        .is_some_and(|row| !row.execution.compiler_owned())
                })
            {
                // This value belongs to an actual selected closure. Keep its
                // exact initializer for the product scalar evaluator.
                program
                    .tables
                    .const_declarations
                    .get_mut(handle)
                    .pending_target_observation = true;
                return Ok(());
            }
            let values = invocation
                .invoke_recorded_in_targets(&targets, accounting)
                .map_err(super::invocation_error)?;
            let carrier = program.tables.const_declarations.get(handle).declared_type;
            // A constant is one value for every deployment that reads it. Each
            // target's layout decides only where the bytes go, so the
            // semantic snapshot must be the same under every one of them.
            let mut folded = None;
            for value in &values {
                let value = decoded(program, carrier, value).map_err(super::invocation_error)?;
                let encoding =
                    language_semantics::const_value::CanonicalConstIdentity::from_declaration_value(
                        &value,
                    )
                    .ok_or_else(|| {
                        super::invocation_error(
                            "constant result has no canonical declaration encoding",
                        )
                    })?
                    .encoding;
                match &folded {
                    None => folded = Some((value, encoding)),
                    Some((_, first)) if *first == encoding => {}
                    Some(_) => {
                        return Err(super::invocation_error(
                            "constant result differs between the deployments that read it",
                        ));
                    }
                }
            }
            let (value, encoding) = folded
                .ok_or_else(|| super::invocation_error("constant evaluation returned no result"))?;
            let node = program
                .materialize_canonical_const_value(carrier, &value)
                .ok_or_else(|| {
                    super::invocation_error(
                        "constant result does not materialize in its declared carrier",
                    )
                })?;
            let expression = program.expression_table.insert(node);
            let span = program
                .tables
                .const_declarations
                .get(handle)
                .initializer_source_span;
            program.expression_table.set_source_span(expression, span);
            let declaration = program.tables.const_declarations.get_mut(handle);
            declaration.materialized_initializer = expression;
            declaration.evaluated_value = Some(encoding.into());
            if let Some(index) = family_position(&constants, symbol) {
                materialize_uses(program, symbol, &uses[index]).map_err(super::invocation_error)?;
            }
            Ok(())
        })();
        if let Err(mut diagnostics) = result {
            let name = program.symbols.name(symbol);
            let declaration_span = program.symbols.symbol_source_span(symbol);
            for error in diagnostics
                .iter_mut()
                .filter(|diagnostic| diagnostic.is_error())
            {
                error.message = format!("constant `{name}` evaluation: {}", error.message);
                // Name the request while pointing at the operation that failed.
                if error.resolved_location.is_none()
                    && error.source_span.unwrap_or_default() == source::SourceSpan::default()
                {
                    error.source_span = declaration_span;
                }
            }
            return Err(diagnostics);
        }
        completed += 1;
        for &successor in &successors[position] {
            remaining[successor] -= 1;
            if remaining[successor] == 0 {
                ready.push_back(successor);
            }
        }
    }
    if completed != jobs.len() {
        return Err(vec![Diagnostic::error(
            "constant evaluation dependency cycle",
        )]);
    }
    Ok(demanded)
}
