//! Read returned access rules and check their type and function descriptions.
//! Receipts retain the original query, arguments and returned value. Matching
//! those descriptions grants no permission to call functions or access storage.

use access_plans::{
    AccessExposure, AccessPermissions, AccessPlan, AtomicPermissions, BoundaryReach,
    BoundaryServiceReachId, ObservationModel,
};
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use language_semantics::reflection::{
    CallableDescription, FieldDescription, SchemaNode, SchemaNodeHandle, SchemaQueryAuthority,
    SemanticSchemaGraph,
};
use typed_trees::references::StaticArgumentCall;
use typed_trees::types::TypeReferenceHandle;

use crate::{BuildTimeValue, PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt};

type Fields = [(String, BuildTimeValue)];

fn record<'v>(value: &'v BuildTimeValue, names: &[&str]) -> Result<&'v Fields, String> {
    let BuildTimeValue::Struct { fields, .. } = value else {
        return Err("access-plan value is not its declared record".into());
    };
    roster(fields, names)?;
    Ok(fields)
}

fn roster(fields: &Fields, names: &[&str]) -> Result<(), String> {
    if fields.len() != names.len()
        || names.iter().any(|name| {
            fields
                .iter()
                .filter(|(key, _)| key.as_str() == *name)
                .count()
                != 1
        })
    {
        return Err("access-plan record differs from its complete declared field roster".into());
    }
    Ok(())
}

fn field<'v>(fields: &'v Fields, name: &str) -> Result<&'v BuildTimeValue, String> {
    let mut values = fields.iter().filter(|(key, _)| key == name);
    let value = &values
        .next()
        .ok_or_else(|| format!("access-plan value lost `{name}`"))?
        .1;
    if values.next().is_some() {
        return Err(format!("access-plan value repeats `{name}`"));
    }
    Ok(value)
}

fn array(value: &BuildTimeValue) -> Result<&[BuildTimeValue], String> {
    match value {
        BuildTimeValue::Array(values) => Ok(values),
        _ => Err("access-plan value lost its complete sequence".into()),
    }
}

fn bytes(value: &BuildTimeValue) -> Result<&[u8], String> {
    match value {
        BuildTimeValue::Text(value) => Ok(value),
        _ => Err("declaration description lost its immutable byte contents".into()),
    }
}

fn word(value: &BuildTimeValue) -> Result<u64, String> {
    match value {
        BuildTimeValue::Int(value) => Ok(*value as u64),
        _ => Err("access-plan value lost its unsigned scalar carrier".into()),
    }
}

fn boolean(value: &BuildTimeValue) -> Result<bool, String> {
    match value {
        BuildTimeValue::Bool(value) => Ok(*value),
        _ => Err("access-plan value lost its Boolean carrier".into()),
    }
}

fn case(value: &BuildTimeValue) -> Result<(&str, &Fields), String> {
    match value {
        BuildTimeValue::Case { variant, payload } => Ok((variant, payload)),
        _ => Err("access-plan value lost its declared alternative".into()),
    }
}

fn materialized(receipt: &PureTerminalEvaluationReceipt) -> Result<&BuildTimeValue, String> {
    match receipt.outcome() {
        PureTerminalEvaluationOutcome::Materialized { value, .. } => Ok(value),
        _ => Err("semantic plan normalization requires the actual materialized result".into()),
    }
}

fn is_type_schema(source: &typed_trees::TypedTrees, reference: TypeReferenceHandle) -> bool {
    source
        .closed_data_definition(reference)
        .is_some_and(|data| {
            source.core_vocabulary(data.symbol)
                == Some(CoreVocabulary::Reflection(
                    ReflectionDeclaration::TypeSchema,
                ))
        })
}

// Consume the existing query owner's image, under its real lexical author.
// Its private receipt binds the checked query, code and materialized result;
// decoding retains that derivation instead of rerunning source construction.
fn query_schema(
    query: &PureTerminalEvaluationReceipt,
) -> Result<(SemanticSchemaGraph, symbols::SymbolHandle), String> {
    query.validate_charge()?;
    if !query.arguments().is_empty() {
        return Err("semantic schema query has unexpected runtime arguments".into());
    }
    let source = query.source();
    let call = query.call()?;
    let declaration = source
        .machine_by_symbol(call.target_symbol)
        .or_else(|| source.machine_holding_state(call.target_symbol))
        .ok_or("schema result lost its actual query declaration")?;
    if source.core_vocabulary(declaration.symbol)
        != Some(CoreVocabulary::Reflection(ReflectionDeclaration::Schema))
    {
        return Err("access-plan schema is not the actual core declaration query".into());
    }
    if call.authored_machine_application.is_valid()
        && !source
            .expression_table
            .static_machine_application_is_valid(call.authored_machine_application)
    {
        return Err("schema query lost its retained application generation".into());
    }
    let request =
        StaticArgumentCall::Expression(typed_trees::expression::ExpressionHandle::invalid(), call);
    let [operand] = request.arguments(source) else {
        return Err("schema query lost its exact type operand".into());
    };
    if !source
        .type_reference_table
        .contains_type_reference(operand.type_reference)
        || operand.const_literal.is_some()
        || operand.evidence_projection.is_some()
        || operand
            .application
            .as_ref()
            .is_some_and(|application| application.member.is_valid())
    {
        return Err("schema query lost its complete actual type application".into());
    }
    let data = source
        .closed_data_definition(operand.type_reference)
        .ok_or("schema query has no actual closed data declaration")?;
    let mut correspondence = query.program().source_records().iter().filter(|row| {
        row.application
            .as_ref()
            .is_some_and(|application| application.reference == operand.type_reference)
    });
    let row = correspondence
        .next()
        .ok_or("schema query lost its produced source correspondence")?;
    if correspondence.next().is_some() || row.declaration != data.symbol {
        return Err("schema query has ambiguous or foreign source correspondence".into());
    }
    let author = request.lexical_owner(source);
    if !source.symbols.symbols().nodes().is_valid(author) {
        return Err("schema query lost its actual lexical author".into());
    }
    let span = source
        .symbols
        .symbol_provenance_source_span(author)
        .ok_or("schema query lost its author's source custody")?;
    if !source
        .symbols
        .source_reference_may_select_symbol(span, data.symbol)
    {
        return Err("schema query is outside its actual author's declaration scope".into());
    }
    let requester_identity = source.normalized_hermetic_symbol_identity(author)?;
    let authority = if source
        .symbols
        .same_symbol_source_package(author, data.symbol)
    {
        SchemaQueryAuthority::OwningScope { requester_identity }
    } else {
        SchemaQueryAuthority::ForeignScope { requester_identity }
    };
    let supplied = record(materialized(query)?, &["storage"])?;
    let graph = SemanticSchemaGraph::from_runtime_image(bytes(field(supplied, "storage")?)?)?;
    if graph.authority() != &authority {
        return Err("schema image has another actual lexical query scope".into());
    }
    let (identity, hermetic) =
        typed_trees::reflection::construction::exact_symbol_identity(source, data.symbol)?;
    let application = typed_trees::reflection::construction::schema_type_identity(
        source,
        operand.type_reference,
        data.symbol,
    )?;
    let declaration = graph.declaration();
    if declaration.owner_identity != identity
        || declaration.identity_is_hermetic != hermetic
        || declaration.application_identity.as_deref() != Some(application.as_str())
    {
        return Err("schema image root differs from its actual selected source application".into());
    }
    Ok((graph, data.symbol))
}

fn callable(value: &BuildTimeValue) -> Result<CallableDescription, String> {
    let (kind, payload) = case(value)?;
    let requirement = match kind {
        "Machine" => false,
        "Requirement" => true,
        _ => return Err("callable description has another declaration kind".into()),
    };
    roster(
        payload,
        &[
            "declaration_identity",
            "callable_identity",
            "source_revision",
        ],
    )?;
    Ok((
        requirement,
        [
            bytes(field(payload, "declaration_identity")?)?.to_vec(),
            bytes(field(payload, "callable_identity")?)?.to_vec(),
            bytes(field(payload, "source_revision")?)?.to_vec(),
        ],
    ))
}

fn callers(value: &BuildTimeValue) -> Result<Vec<CallableDescription>, String> {
    array(value)?.iter().map(callable).collect()
}

fn described_field(
    graph: &SemanticSchemaGraph,
    revision: &[u8],
    members: &[(SchemaNodeHandle, &FieldDescription)],
    value: &BuildTimeValue,
) -> Result<SchemaNodeHandle, String> {
    let supplied = record(
        value,
        &[
            "member_identity",
            "case_identity",
            "owner_identity",
            "owner_application",
            "name",
            "qualified_type",
            "declaration_position",
            "has_stable_number",
            "is_erased",
            "stable_number",
            "source_revision",
        ],
    )?;
    let declaration = graph.declaration();
    let application = declaration
        .application_identity
        .as_deref()
        .ok_or("semantic schema lost its actual owner application")?;
    if bytes(field(supplied, "owner_identity")?)? != declaration.owner_identity.as_bytes()
        || bytes(field(supplied, "owner_application")?)? != application.as_bytes()
        || bytes(field(supplied, "source_revision")?)? != revision
    {
        return Err(
            "field description belongs to another owner, application or source revision".into(),
        );
    }
    let identity = bytes(field(supplied, "member_identity")?)?;
    let index = members
        .binary_search_by(|(_, field)| field.member_identity.as_bytes().cmp(identity))
        .map_err(|_| "field description is outside the actual schema subject")?;
    let (handle, expected) = members[index];
    let case_identity = if expected.owner_case.is_valid() {
        let SchemaNode::Case(case) = graph.node(expected.owner_case) else {
            return Err("field description lost its actual case owner".into());
        };
        case.member_identity.as_bytes()
    } else {
        &[]
    };
    if bytes(field(supplied, "case_identity")?)? != case_identity
        || bytes(field(supplied, "name")?)? != expected.name.as_bytes()
        || bytes(field(supplied, "qualified_type")?)? != expected.qualified_type.as_bytes()
        || word(field(supplied, "declaration_position")?)?
            != u64::from(expected.declaration_position)
        || boolean(field(supplied, "has_stable_number")?)? != expected.stable_number.is_some()
        || word(field(supplied, "stable_number")?)? != expected.stable_number.unwrap_or(0)
        || boolean(field(supplied, "is_erased")?)? != expected.erased
    {
        return Err("field description changed its exact member or case correspondence".into());
    }
    Ok(handle)
}

fn region(
    schema_value: &BuildTimeValue,
    graph: &SemanticSchemaGraph,
    revision: &[u8],
    members: &[(SchemaNodeHandle, &FieldDescription)],
    value: &BuildTimeValue,
) -> Result<SchemaNodeHandle, String> {
    let (kind, payload) = case(value)?;
    match kind {
        "Subject" => {
            roster(payload, &["schema"])?;
            if field(payload, "schema")? != schema_value {
                return Err("whole-subject rule describes another actual schema".into());
            }
            Ok(graph.root())
        }
        "Field" => {
            roster(payload, &["field"])?;
            described_field(graph, revision, members, field(payload, "field")?)
        }
        _ => Err("access rule has another semantic region kind".into()),
    }
}

fn exposure(value: &BuildTimeValue) -> Result<AccessExposure, String> {
    let (kind, payload) = case(value)?;
    roster(payload, &[])?;
    match kind {
        "Exported" => Ok(AccessExposure::Exported),
        "BindingPrivate" => Ok(AccessExposure::BindingPrivate),
        _ => Err("observation demand has another exposure".into()),
    }
}

fn observation(
    value: &BuildTimeValue,
) -> Result<Option<(ObservationModel, AccessPermissions, AccessExposure)>, String> {
    let (kind, payload) = case(value)?;
    let mut permissions = AccessPermissions::default();
    let model = match kind {
        "Inaccessible" => {
            roster(payload, &[])?;
            return Ok(None);
        }
        "Stable" => {
            roster(payload, &["read", "write", "exposure"])?;
            permissions.read = boolean(field(payload, "read")?)?;
            permissions.write = boolean(field(payload, "write")?)?;
            ObservationModel::Stable
        }
        "External" => {
            roster(payload, &["read", "write", "exposure"])?;
            let (read, empty) = case(field(payload, "read")?)?;
            roster(empty, &[])?;
            match read {
                "None" => {}
                "Read" => permissions.read = true,
                "Take" => permissions.take = true,
                _ => return Err("external observation lost its read behavior".into()),
            }
            permissions.write = boolean(field(payload, "write")?)?;
            ObservationModel::External
        }
        "Atomic" => {
            roster(payload, &["operations", "exposure"])?;
            let operations = record(
                field(payload, "operations")?,
                &[
                    "load",
                    "store",
                    "fetch_add",
                    "fetch_sub",
                    "fetch_xor",
                    "fetch_or",
                    "fetch_and",
                    "swap",
                    "compare_exchange",
                    "compare_exchange_once",
                    "try_exchange",
                    "try_exchange_once",
                ],
            )?;
            permissions.atomic = AtomicPermissions {
                load: boolean(field(operations, "load")?)?,
                store: boolean(field(operations, "store")?)?,
                fetch_add: boolean(field(operations, "fetch_add")?)?,
                fetch_sub: boolean(field(operations, "fetch_sub")?)?,
                fetch_xor: boolean(field(operations, "fetch_xor")?)?,
                fetch_or: boolean(field(operations, "fetch_or")?)?,
                fetch_and: boolean(field(operations, "fetch_and")?)?,
                swap: boolean(field(operations, "swap")?)?,
                compare_exchange: boolean(field(operations, "compare_exchange")?)?,
                compare_exchange_once: boolean(field(operations, "compare_exchange_once")?)?,
                try_exchange: boolean(field(operations, "try_exchange")?)?,
                try_exchange_once: boolean(field(operations, "try_exchange_once")?)?,
            };
            ObservationModel::Atomic
        }
        _ => return Err("access plan has another observation demand".into()),
    };
    Ok(Some((
        model,
        permissions,
        exposure(field(payload, "exposure")?)?,
    )))
}

fn active<'v>(fields: &'v Fields, name: &str, count: &str) -> Result<&'v [BuildTimeValue], String> {
    let values = array(field(fields, name)?)?;
    // These are the existing core private construction capacities. They are
    // checked for complete transport, never used as semantic field identity.
    if values.len() != 32 {
        return Err(format!(
            "access-plan `{name}` lost its complete 32-element storage"
        ));
    }
    let count = usize::try_from(word(field(fields, count)?)?)
        .map_err(|_| "access-plan count exceeds this host")?;
    values
        .get(..count)
        .ok_or_else(|| format!("access-plan `{name}` exceeds its actual construction storage"))
}

/// Normalize data from the existing bound receipts. This grants no caller or
/// issuer permission; policy admission still needs the exact checked public
/// invocation and selected application before any consumer may use the result.
pub(crate) fn normalize(
    query: &PureTerminalEvaluationReceipt,
    plan: &PureTerminalEvaluationReceipt,
) -> Result<AccessPlan, String> {
    if plan.admission().target() != query.admission().target() {
        return Err("semantic plan and query belong to different invocation targets".into());
    }
    let (graph, owner) = query_schema(query)?;
    let revision =
        typed_trees::reflection::construction::declaration_source_revision(query.source(), owner)?;
    normalize_for_schema(plan, graph, materialized(query)?, &revision)
}

/// The query or domain request owner supplies the schema it actually passed.
/// Reuse that completed graph and the invocation's own bound argument/result;
/// normalization does not reconstruct source or grant operation authority.
pub(super) fn normalize_for_schema(
    plan: &PureTerminalEvaluationReceipt,
    graph: SemanticSchemaGraph,
    schema_value: &BuildTimeValue,
    revision: &[u8],
) -> Result<AccessPlan, String> {
    plan.validate_charge()?;
    let source = plan.source();
    // The accepted caller declares the actual completed output. Describing
    // that output does not choose a public requirement or authorize its body.
    let caller = source
        .machine_by_symbol(plan.caller())
        .ok_or("semantic plan lost its actual checked caller")?;
    let Some([entry]) = source.machine_states(caller).as_array() else {
        return Err("semantic plan lost its unique checked caller entry".into());
    };
    if !source
        .type_reference_table
        .contains_type_reference(entry.return_type)
    {
        return Err("semantic plan caller lost its declared result type".into());
    }
    let result = source
        .closed_data_definition(entry.return_type)
        .ok_or("access-plan result has no actual data declaration")?;
    if source.core_vocabulary(result.symbol) != Some(CoreVocabulary::AccessPlan) {
        return Err("semantic plan result is not the genuine core AccessPlan declaration".into());
    }
    let arguments = source
        .expression_table
        .expression_handles(plan.call()?.arguments);
    if arguments.len() != plan.arguments().len() {
        return Err("semantic plan lost its complete actual argument roster".into());
    }
    let mut supplied_schema = None;
    for (expression, value) in arguments.iter().zip(plan.arguments()) {
        let reference = source
            .expression_table
            .constructed_value_type_reference(*expression);
        if !source
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("semantic plan argument lost its actual constructed type".into());
        }
        if is_type_schema(source, reference) && supplied_schema.replace(value).is_some() {
            return Err("access-plan call has no unique actual TypeSchema argument".into());
        }
    }
    let supplied_schema =
        supplied_schema.ok_or("access-plan call has no unique actual TypeSchema argument")?;
    if supplied_schema != schema_value {
        return Err(
            "access-plan checked arguments do not contain the actual schema query value".into(),
        );
    }
    let mut members = graph.governed_fields()?;
    members.sort_unstable_by(|left, right| left.1.member_identity.cmp(&right.1.member_identity));
    if members
        .windows(2)
        .any(|pair| pair[0].1.member_identity == pair[1].1.member_identity)
    {
        return Err("actual schema repeats a semantic member identity".into());
    }
    let fields = record(
        materialized(plan)?,
        &[
            "schema",
            "issuance",
            "fields",
            "field_count",
            "readers",
            "reader_count",
            "writers",
            "writer_count",
            "reach",
        ],
    )?;
    if field(fields, "schema")? != schema_value {
        return Err("returned semantic plan describes another schema value".into());
    }
    let (choice, payload) = case(field(fields, "issuance")?)?;
    let issuance = match choice {
        "Only" => {
            roster(payload, &["routes"])?;
            Some(callers(field(payload, "routes")?)?)
        }
        "Unrestricted" => {
            roster(payload, &[])?;
            None
        }
        _ => return Err("semantic plan has no explicit issuance choice".into()),
    };
    let rules = |name, count| -> Result<Vec<_>, String> {
        active(fields, name, count)?
            .iter()
            .map(|value| {
                let row = record(value, &["region", "callers"])?;
                Ok((
                    region(
                        schema_value,
                        &graph,
                        revision,
                        &members,
                        field(row, "region")?,
                    )?,
                    callers(field(row, "callers")?)?,
                ))
            })
            .collect()
    };
    let readers = rules("readers", "reader_count")?;
    let writers = rules("writers", "writer_count")?;
    let observations = active(fields, "fields", "field_count")?
        .iter()
        .map(|value| {
            let row = record(value, &["field", "access"])?;
            Ok((
                described_field(&graph, revision, &members, field(row, "field")?)?,
                observation(field(row, "access")?)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let reach = record(field(fields, "reach")?, &["services", "service_count"])?;
    let services = active(reach, "services", "service_count")?;
    let reach = BoundaryReach::from_services(
        services
            .iter()
            .map(|value| {
                BoundaryServiceReachId::from_normalized_identity(word(value)?)
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?,
    );
    let mut described = issuance
        .iter()
        .flat_map(|rows| rows.iter())
        .cloned()
        .collect::<Vec<_>>();
    described.extend(
        readers
            .iter()
            .chain(&writers)
            .flat_map(|(_, values)| values)
            .cloned(),
    );
    typed_trees::reflection::construction::authenticate_callable_descriptions(source, &described)?;
    access_plans::normalize_access_plan(AccessPlan {
        schema: graph,
        issuance,
        readers,
        writers,
        observations,
        reach,
    })
    .map_err(|error| error.to_string())
}
