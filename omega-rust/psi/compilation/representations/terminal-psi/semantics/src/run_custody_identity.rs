//! Compiler run operations must retain their exact bundled declaration and
//! complete contract after receiving. Descriptor geometry identifies fields;
//! it does not authorize another callable to acquire compiler custody.

use language_core::sealed_intrinsics::{
    CompilerPrimitive, RunCustodyOperation, SealedDeclarationSite, SealedIntrinsic,
    SealedIntrinsicCatalog,
};
use language_semantics::type_identity::encoding::{CoordinateTag, atom, bare, byte_atom, compound};
use language_semantics::type_identity::vocabulary::{
    ConstraintForm, Label, NameForm, OwnerForm, TypeForm,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerMathTerm as Math, IntegerSign, IntegerType,
    IntegerValue, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, CheckedBoundaryCallable, CheckedBoundaryExecution,
    CheckedMachineDeclaration, ClosedReachArgument, NominalDeclarationIdentity,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    TerminalIdentityTable,
};

use crate::{RunCustodyContract, RunDescriptorRoles};

struct SourceCoordinates {
    owner: String,
    entry: String,
    semantic: String,
    callable: String,
    parameters: Vec<String>,
    result: String,
}

fn path(operation: RunCustodyOperation) -> &'static str {
    match operation {
        RunCustodyOperation::ReleaseEmpty => "Run::retire_empty",
        RunCustodyOperation::Split => "Run::split",
        RunCustodyOperation::Merge => "Run::merge",
    }
}

fn core_name(path: &str) -> String {
    compound(
        NameForm::Nominal,
        [bare(OwnerForm::Toolchain), atom(Label::Path, path)],
    )
}
fn named(name: &str) -> String {
    compound(TypeForm::Named, [atom(Label::Name, name)])
}
fn scalar(name: &str) -> String {
    named(&compound(NameForm::CompilerType, [atom(Label::Atom, name)]))
}
fn generic(path: &str, arguments: &[&str]) -> String {
    compound(
        TypeForm::Generic,
        std::iter::once(atom(Label::Name, &core_name(path)))
            .chain(arguments.iter().map(|argument| (*argument).to_owned())),
    )
}
fn qualified(base: String, domains: &[(&str, &[&str])]) -> String {
    // The ordinary canonical normalizer orders the domain identities before
    // wrapping their declared-domain constraint, and retains both layers.
    let mut domains = domains
        .iter()
        .map(|(path, arguments)| {
            compound(
                ConstraintForm::DeclaredDomain,
                std::iter::once(atom(Label::Name, &core_name(path)))
                    .chain(arguments.iter().map(|argument| (*argument).to_owned())),
            )
        })
        .collect::<Vec<_>>();
    domains.sort_unstable();
    compound(
        TypeForm::Constrained,
        std::iter::once(base).chain(
            domains.into_iter().map(|domain| {
                compound(ConstraintForm::DeclaredDomain, [atom(Label::Name, &domain)])
            }),
        ),
    )
}
fn live_run(policy: &str, element: &str) -> String {
    let arguments = [policy, element];
    qualified(generic("Run", &arguments), &[("Live", &arguments)])
}
fn source_digest() -> Result<[u8; 32], &'static str> {
    static DIGEST: std::sync::OnceLock<Result<[u8; 32], &'static str>> = std::sync::OnceLock::new();
    *DIGEST.get_or_init(|| {
        let (file, bytes) = language_core::sealed_intrinsics::run_source();
        source::source_identity::toolchain_source_identity_digest(&source::SourceFile {
            source_id: source::SourceId(0),
            path: std::path::Path::new("/compiler/core").join(file),
            package_root: "/compiler/core".into(),
            package_identity: None,
            dependency_scope: source::DependencyScope::Product,
            origin: source::SourceOrigin::Toolchain,
            resolution_stratum: source::SourceResolutionStratum::Base,
            source: std::str::from_utf8(bytes)
                .map_err(|_| "run catalog source is not UTF-8")?
                .into(),
        })
        .map_err(|_| "run catalog source has no exact identity")
    })
}
fn coordinates(
    operation: RunCustodyOperation,
    policy: &str,
    element: &str,
) -> Result<SourceCoordinates, &'static str> {
    let declaration = path(operation);
    let (file, bytes) = language_core::sealed_intrinsics::run_source();
    if SealedIntrinsicCatalog::classify(SealedDeclarationSite {
        file: std::path::Path::new(file),
        declaration,
        source: bytes,
    }) != Some(SealedIntrinsic::CompilerPrimitive(
        CompilerPrimitive::RunCustody(operation),
    )) {
        return Err("run operation has no exact admitted compiler declaration");
    }
    let digest = source_digest()?;
    let exact = |path: &str| {
        compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::ToolchainSource, &digest),
                atom(Label::Path, path),
            ],
        )
    };
    let binder_p = named("$T0");
    let binder_t = named("$T1");
    let formal = live_run(&binder_p, &binder_t);
    let template_parameters = match operation {
        RunCustodyOperation::ReleaseEmpty => vec![formal],
        RunCustodyOperation::Split => vec![formal, named("u64")],
        RunCustodyOperation::Merge => vec![formal.clone(), formal],
    };
    let parameters = template_parameters
        .iter()
        .map(|ty| {
            compound(
                CoordinateTag::Parameter,
                [
                    atom(CoordinateTag::Mutable, "no"),
                    atom(CoordinateTag::Const, "no"),
                    ty.clone(),
                ],
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let callable = compound(
        CoordinateTag::NamedCallable,
        [
            atom(CoordinateTag::Path, &core_name(declaration)),
            atom(CoordinateTag::Parameters, &parameters),
            atom(CoordinateTag::ResultDispatch, ""),
        ],
    );
    let run = live_run(policy, element);
    let parameters = match operation {
        RunCustodyOperation::ReleaseEmpty => vec![run.clone()],
        RunCustodyOperation::Split => vec![run.clone(), scalar("u64")],
        RunCustodyOperation::Merge => vec![run.clone(), run.clone()],
    };
    let result = match operation {
        RunCustodyOperation::ReleaseEmpty => qualified(
            named(&core_name("Extent")),
            &[("Extent::Granted", &[]), ("Extent::Vacant", &[])],
        ),
        RunCustodyOperation::Split => generic("RunParts", &[policy, element]),
        RunCustodyOperation::Merge => run,
    };
    Ok(SourceCoordinates {
        owner: exact(declaration),
        entry: exact(&format!(
            "{declaration}::{}",
            declaration
                .rsplit("::")
                .next()
                .ok_or("run catalog declaration has no entry")?
        )),
        semantic: format!("toolchain::{declaration}"),
        callable,
        parameters,
        result,
    })
}

struct ActualTypes<'a> {
    operation: RunCustodyOperation,
    policy: &'a str,
    element: &'a str,
}

fn static_source<'a>(
    identities: &'a TerminalIdentityTable,
    owner: &NominalDeclarationIdentity,
    identity: TerminalIdentityIndex,
    checked: &'a CheckedBoundaryCallable,
) -> Result<ActualTypes<'a>, &'static str> {
    let CheckedBoundaryExecution::RunCustody {
        operation, policy, ..
    } = &checked.execution
    else {
        return Err("run source authentication needs its actual compiler execution");
    };
    let [ClosedReachArgument::Type(p), ClosedReachArgument::Type(t)] =
        checked.method_arguments.as_slice()
    else {
        return Err("run compiler source lost its complete P and T application");
    };
    let text = |identity| {
        identities
            .text(identity)
            .filter(|text| !text.is_empty())
            .ok_or("run compiler source has an unresolved identity coordinate")
    };
    if policy != p {
        return Err("run execution policy differs from its actual P type operand");
    }
    let actual = ActualTypes {
        operation: *operation,
        policy: text(*p)?,
        element: text(*t)?,
    };
    let expected = coordinates(*operation, actual.policy, actual.element)?;
    let source = &checked.source;
    let specialization = source
        .specialization
        .as_ref()
        .ok_or("run compiler source has no exact closed specialization")?;
    if text(owner.identity())? != expected.owner
        || text(source.machine.identity())? != expected.owner
        || text(source.state.identity())? != expected.entry
        || text(source.semantic_declaration)? != expected.semantic
        || text(specialization.template.identity())? != expected.owner
        || text(specialization.template_entry.identity())? != expected.entry
        || text(specialization.template_semantic_declaration)? != expected.semantic
        || text(specialization.template_callable)? != expected.callable
        || specialization.capture.is_some()
    {
        return Err("run compiler source differs from its exact bundled owner or template");
    }
    let mut commitment =
        language_semantics::conformance_identity::MachineApplicationCommitment::new(
            &expected.callable,
        )?;
    commitment.type_argument(actual.policy)?;
    commitment.type_argument(actual.element)?;
    let commitment = commitment.finish();
    let callable = CheckedMachineDeclaration::callable_evidence_identity(
        &expected.callable,
        Some(&commitment),
    );
    if specialization.commitment != commitment
        || text(source.callable)? != callable
        || identity != source.callable
    {
        return Err("run compiler source changed its complete type specialization");
    }
    if source.parameter_types.len() != expected.parameters.len()
        || source
            .parameter_types
            .iter()
            .zip(&expected.parameters)
            .any(|(actual, expected)| identities.text(*actual) != Some(expected.as_str()))
        || text(source.result_type)? != expected.result
    {
        return Err("run compiler source changed its actual parameter or result types");
    }
    if source.contract == [0; 32]
        || checked.service_declaration != source.contract
        || source.published_may_suspend
        || source.published_may_block
    {
        return Err("run compiler source lost its complete total contract commitment");
    }
    Ok(actual)
}

fn scalar_name(scalar: ScalarType) -> Result<String, &'static str> {
    Ok(match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::Integer(integer) if integer.is_address() => "addr".to_owned(),
        ScalarType::Integer(integer) => format!(
            "{}{}",
            if integer.sign() == IntegerSign::Unsigned {
                "u"
            } else {
                "i"
            },
            integer
                .bits()
                .ok_or("run element has an open integer width")?
        ),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64".to_owned(),
    })
}

// Keep the original qualified identity in the source and specialization. This
// conversion serves only the exact ordinary storage identity comparison. An
// unserved qualification/generic normalization refuses rather than erasing it.
fn ordinary_element<'a>(
    actual: &str,
    element: &StructuralFieldType,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
) -> Result<String, &'static str> {
    match element {
        StructuralFieldType::Scalar(kind) => {
            let name = scalar_name(*kind)?;
            if actual != scalar(&name) {
                return Err("run scalar element needs its exact qualified storage correspondence");
            }
            Ok(named(&name))
        }
        StructuralFieldType::Structural(id) => {
            let declaration =
                lookup(*id).ok_or("run element lost its actual structural declaration")?;
            if actual != declaration.identity {
                return Err(
                    "run structural element needs its exact qualified storage correspondence",
                );
            }
            Ok(declaration.identity.clone())
        }
        _ => Err("run element has no admitted ordinary storage correspondence"),
    }
}

fn require_record_identity(
    declaration: &StructuralTypeDeclaration,
    expected: &str,
    nominal: &str,
    identities: &TerminalIdentityTable,
) -> Result<(), &'static str> {
    if declaration.identity != expected
        || identities.text(declaration.declaration_identity) != Some(nominal)
    {
        return Err("run descriptor differs from its complete nominal P and T application");
    }
    Ok(())
}

fn release_roles<'module>(
    boundary: &BoundaryMachineDeclaration,
    contract: &crate::EmptyRunReleaseContract<'module>,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'module StructuralTypeDeclaration>,
) -> Result<RunDescriptorRoles<'module>, &'static str> {
    let parameter = boundary
        .structural_parameters
        .first()
        .ok_or("empty run lost its exact structural input")?;
    let carrier = lookup(parameter.structural_type).ok_or("empty run lost its actual carrier")?;
    let StructuralTypeShape::Record { fields, .. } = &carrier.shape else {
        return Err("empty run is not a descriptor record");
    };
    let [storage, count] = fields.as_slice() else {
        return Err("empty run lost its checked descriptor fields");
    };
    let extent = storage
        .field_type
        .as_structural()
        .ok_or("empty run backing is not structural")?;
    let backing = lookup(extent).ok_or("empty run lost its checked backing declaration")?;
    let StructuralTypeShape::Record { fields, .. } = &backing.shape else {
        return Err("empty run backing is not a descriptor record");
    };
    let [base, length] = fields.as_slice() else {
        return Err("empty run lost its checked geometry fields");
    };
    if storage.identity != contract.storage
        || count.id != contract.count
        || length.id != contract.length
        || extent != contract.result.structural_type
    {
        return Err("empty run scalar contract differs from its projected descriptor roles");
    }
    Ok(RunDescriptorRoles {
        carrier: carrier.id,
        extent,
        storage,
        count,
        base,
        length,
    })
}

/// The caller prepares type lookup once for the containing declaration family.
/// These checks authenticate source meaning only; the actual ordered live-cell
/// custody, placement, loans and constant-cost execution remain independent.
pub(crate) fn validate<'module>(
    module: &terminal_psi::TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    contract: &RunCustodyContract<'module>,
    mut lookup: impl FnMut(StructuralTypeId) -> Option<&'module StructuralTypeDeclaration>,
) -> Result<(), &'static str> {
    let checked = boundary
        .declaration
        .as_ref()
        .ok_or("run compiler operation has no checked source")?;
    let actual = static_source(
        &module.identities,
        &boundary.owner,
        boundary.identity,
        checked,
    )?;
    let (run, parts) = match (actual.operation, contract) {
        (RunCustodyOperation::ReleaseEmpty, RunCustodyContract::ReleaseEmpty(contract)) => {
            (release_roles(boundary, contract, &mut lookup)?, None)
        }
        (
            RunCustodyOperation::Split,
            RunCustodyContract::Split {
                run,
                left,
                right,
                result,
            },
        ) => (*run, Some((left.id, right.id, result.structural_type))),
        (RunCustodyOperation::Merge, RunCustodyContract::Merge { run, .. }) => (*run, None),
        _ => return Err("run compiler source changed its projected operation"),
    };
    let CheckedBoundaryExecution::RunCustody { element, .. } = &checked.execution else {
        unreachable!("static source authenticated the same execution")
    };
    let ordinary_t = ordinary_element(actual.element, element, &mut lookup)?;
    let run_declaration = lookup(run.carrier).ok_or("run lost its actual carrier declaration")?;
    require_record_identity(
        run_declaration,
        &generic("Run", &[actual.policy, &ordinary_t]),
        "toolchain::Run",
        &module.identities,
    )?;
    let extent = lookup(run.extent).ok_or("run lost its actual backing declaration")?;
    require_record_identity(
        extent,
        &named(&core_name("Extent")),
        "toolchain::Extent",
        &module.identities,
    )?;
    if let Some((_, _, id)) = parts {
        require_record_identity(
            lookup(id).ok_or("run split lost its actual parts declaration")?,
            &generic("RunParts", &[actual.policy, &ordinary_t]),
            "toolchain::RunParts",
            &module.identities,
        )?;
    }
    for (ordinal, parameter) in boundary.structural_parameters.iter().enumerate() {
        if parameter.place
            != PlaceId::new(ordinal as u64 + 1).ok_or("run formal coordinate overflow")?
        {
            return Err("run compiler source changed its exact formal storage coordinates");
        }
    }
    let fields = FieldRoles {
        storage: run.storage.id,
        count: run.count.id,
        base: run.base.id,
        length: run.length.id,
        address: run
            .base
            .field_type
            .scalar_type()
            .and_then(|ty| ty.as_integer())
            .ok_or("run base has no checked address scalar")?,
    };
    let (requires, ensures) = scalar_contract(
        actual.operation,
        fields,
        parts.map(|(left, right, _)| (left, right)),
    )?;
    if !same_clauses(&boundary.scalar_requires, requires)
        || !same_clauses(&boundary.scalar_ensures, ensures)
    {
        return Err("run compiler source changed its complete scalar requires or ensures");
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct FieldRoles {
    storage: StructuralFieldId,
    count: StructuralFieldId,
    base: StructuralFieldId,
    length: StructuralFieldId,
    address: IntegerType,
}

fn canonical(mut proposition: Proposition) -> Proposition {
    match &mut proposition {
        Proposition::Equal(left, right) if left > right => std::mem::swap(left, right),
        Proposition::IntegerMathEqual(left, right) if left > right => std::mem::swap(left, right),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            for child in children.iter_mut() {
                *child = canonical(child.clone());
            }
            children.sort_unstable();
        }
        _ => {}
    }
    proposition
}
fn same_clauses(actual: &[Proposition], expected: Vec<Proposition>) -> bool {
    // The source reader canonicalizes equality and alternatives inside each
    // proposition. The authored clause roster itself stays ordered: a call's
    // requirement obligation names that exact row, including any duplicates.
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| canonical(actual.clone()) == canonical(expected))
}
fn or(clauses: Vec<Proposition>) -> Proposition {
    let mut flat = Vec::new();
    for clause in clauses {
        match clause {
            Proposition::Disjunction(inner) => flat.extend(inner),
            other => flat.push(other),
        }
    }
    Proposition::Disjunction(flat)
}
fn ne(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    or(vec![
        Proposition::LessThan(left.clone(), right.clone()),
        Proposition::LessThan(right, left),
    ])
}
fn equal(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::Equal(left, right)
}
fn math_equal(left: Math, right: Math) -> Proposition {
    Proposition::IntegerMathEqual(left, right)
}
fn embed(term: ScalarTerm) -> Result<Math, &'static str> {
    Math::embed(term).ok_or("run scalar contract embeds a non-integer")
}
fn sum(left: Math, right: Math) -> Math {
    Math::Add(Box::new(left), Box::new(right))
}
fn product(left: Math, right: Math) -> Math {
    Math::Multiply(Box::new(left), Box::new(right))
}

fn scalar_contract(
    operation: RunCustodyOperation,
    fields: FieldRoles,
    parts: Option<(StructuralFieldId, StructuralFieldId)>,
) -> Result<(Vec<Proposition>, Vec<Proposition>), &'static str> {
    let count_type =
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| "run count carrier is invalid")?;
    let place = |id| PlaceId::new(id).ok_or("run scalar contract has an invalid place coordinate");
    let field = |root, path, scalar_type| {
        Ok::<_, &'static str>(ScalarTerm::integer_field_path(
            place(root)?,
            path,
            scalar_type,
        ))
    };
    let zero = || ScalarTerm::Integer {
        scalar_type: count_type,
        value: IntegerValue::Unsigned(0),
    };
    let count = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.count)])
                .collect(),
            count_type,
        )
    };
    let length = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.storage), Path::Field(fields.length)])
                .collect(),
            count_type,
        )
    };
    let base = |root, prefix: &[Path]| {
        field(
            root,
            prefix
                .iter()
                .cloned()
                .chain([Path::Field(fields.storage), Path::Field(fields.base)])
                .collect(),
            fields.address,
        )
    };
    let extent_length = |root| field(root, vec![Path::Field(fields.length)], count_type);
    let extent_base = |root| field(root, vec![Path::Field(fields.base)], fields.address);
    match operation {
        RunCustodyOperation::ReleaseEmpty => Ok((
            vec![
                equal(count(1, &[])?, zero()),
                equal(length(1, &[])?, zero()),
            ],
            vec![
                equal(extent_base(2)?, base(1, &[])?),
                equal(extent_length(2)?, length(1, &[])?),
            ],
        )),
        RunCustodyOperation::Split => {
            let (left, right) = parts.ok_or("run split lost its actual result field roles")?;
            let left = [Path::Field(left)];
            let right = [Path::Field(right)];
            let cut = ScalarTerm::Value {
                id: ValueId::new(1).ok_or("run cut value coordinate is invalid")?,
                scalar_type: ScalarType::Integer(count_type),
            };
            let run_count = count(1, &[])?;
            let run_length = length(1, &[])?;
            let run_base = base(1, &[])?;
            let left_count = count(2, &left)?;
            let right_count = count(2, &right)?;
            let left_length = length(2, &left)?;
            let right_length = length(2, &right)?;
            let left_base = base(2, &left)?;
            let right_base = base(2, &right)?;
            Ok((
                vec![Proposition::LessOrEqual(cut.clone(), run_count.clone())],
                vec![
                    equal(left_count, cut.clone()),
                    equal(
                        right_count,
                        ScalarTerm::exact_integer_subtract(
                            count_type,
                            run_count.clone(),
                            cut.clone(),
                        )
                        .map_err(|_| "run split count subtraction is malformed")?,
                    ),
                    equal(left_base, run_base.clone()),
                    math_equal(
                        sum(embed(left_length.clone())?, embed(right_length.clone())?),
                        embed(run_length.clone())?,
                    ),
                    math_equal(
                        product(embed(left_length.clone())?, embed(run_count.clone())?),
                        product(embed(run_length)?, embed(cut.clone())?),
                    ),
                    or(vec![
                        ne(cut.clone(), zero()),
                        equal(left_length.clone(), zero()),
                    ]),
                    or(vec![
                        ne(cut, run_count),
                        equal(right_length.clone(), zero()),
                    ]),
                    or(vec![
                        ne(left_length.clone(), zero()),
                        equal(right_base.clone(), run_base.clone()),
                    ]),
                    or(vec![
                        equal(right_length.clone(), zero()),
                        math_equal(
                            embed(right_base.clone())?,
                            sum(embed(run_base.clone())?, embed(left_length)?),
                        ),
                    ]),
                    or(vec![ne(right_length, zero()), equal(right_base, run_base)]),
                ],
            ))
        }
        RunCustodyOperation::Merge => {
            let left_count = count(1, &[])?;
            let right_count = count(2, &[])?;
            let left_length = length(1, &[])?;
            let right_length = length(2, &[])?;
            let left_base = base(1, &[])?;
            let right_base = base(2, &[])?;
            let counts = sum(embed(left_count)?, embed(right_count)?);
            let lengths = sum(embed(left_length.clone())?, embed(right_length.clone())?);
            let maximum = Math::literal(IntegerValue::Unsigned(u64::MAX.into()));
            Ok((
                vec![
                    Proposition::IntegerMathLessOrEqual(counts.clone(), maximum.clone()),
                    Proposition::IntegerMathLessOrEqual(lengths.clone(), maximum),
                    or(vec![
                        equal(left_length.clone(), zero()),
                        equal(right_length, zero()),
                        math_equal(
                            embed(right_base.clone())?,
                            sum(embed(left_base.clone())?, embed(left_length.clone())?),
                        ),
                    ]),
                ],
                vec![
                    math_equal(embed(count(3, &[])?)?, counts),
                    math_equal(embed(length(3, &[])?)?, lengths),
                    or(vec![
                        equal(left_length.clone(), zero()),
                        equal(base(3, &[])?, left_base),
                    ]),
                    or(vec![
                        ne(left_length, zero()),
                        equal(base(3, &[])?, right_base),
                    ]),
                ],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{CheckedMachineSpecialization, TerminalIdentityTableBuilder};

    fn checked(
        operation: RunCustodyOperation,
    ) -> (
        TerminalIdentityTable,
        NominalDeclarationIdentity,
        TerminalIdentityIndex,
        CheckedBoundaryCallable,
    ) {
        let p = named(&compound(
            NameForm::Nominal,
            [
                byte_atom(OwnerForm::Package, &[7; 32]),
                atom(Label::Path, "Policy"),
            ],
        ));
        let t = scalar("u64");
        let expected = coordinates(operation, &p, &t).unwrap();
        let mut identities = TerminalIdentityTableBuilder::default();
        let owner = NominalDeclarationIdentity::new(identities.intern(&expected.owner));
        let state = NominalDeclarationIdentity::new(identities.intern(&expected.entry));
        let semantic = identities.intern(&expected.semantic);
        let template_callable = identities.intern(&expected.callable);
        let mut commitment =
            language_semantics::conformance_identity::MachineApplicationCommitment::new(
                &expected.callable,
            )
            .unwrap();
        commitment.type_argument(&p).unwrap();
        commitment.type_argument(&t).unwrap();
        let commitment = commitment.finish();
        let callable = identities.intern(&CheckedMachineDeclaration::callable_evidence_identity(
            &expected.callable,
            Some(&commitment),
        ));
        let policy = identities.intern(&p);
        let element = identities.intern(&t);
        let checked = CheckedBoundaryCallable {
            service_declaration: [9; 32],
            source: CheckedMachineDeclaration {
                machine: owner.clone(),
                state: state.clone(),
                semantic_declaration: semantic,
                parameter_types: expected
                    .parameters
                    .iter()
                    .map(|ty| identities.intern(ty))
                    .collect(),
                callable,
                result_type: identities.intern(&expected.result),
                contract: [9; 32],
                published_may_suspend: false,
                published_may_block: false,
                specialization: Some(CheckedMachineSpecialization {
                    capture: None,
                    template: owner.clone(),
                    template_entry: state,
                    template_callable,
                    template_semantic_declaration: semantic,
                    commitment,
                }),
            },
            execution: CheckedBoundaryExecution::RunCustody {
                operation,
                policy,
                element: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            },
            method_arguments: vec![
                ClosedReachArgument::Type(policy),
                ClosedReachArgument::Type(element),
            ],
        };
        (identities.snapshot(), owner, callable, checked)
    }

    #[test]
    fn run_receiving_source_refuses_changed_owner_operation_policy_and_specialization() {
        for operation in [
            RunCustodyOperation::ReleaseEmpty,
            RunCustodyOperation::Split,
            RunCustodyOperation::Merge,
        ] {
            let (identities, owner, identity, source) = checked(operation);
            static_source(&identities, &owner, identity, &source).unwrap();
            for fault in 0..7 {
                let mut changed = source.clone();
                match fault {
                    0 => changed.source.state = changed.source.machine.clone(),
                    1 => changed.source.specialization.as_mut().unwrap().commitment[0] ^= 1,
                    2 => changed.method_arguments.swap(0, 1),
                    3 => {
                        if let CheckedBoundaryExecution::RunCustody { policy, .. } =
                            &mut changed.execution
                        {
                            let ClosedReachArgument::Type(element) = changed.method_arguments[1]
                            else {
                                unreachable!()
                            };
                            *policy = element;
                        }
                    }
                    4 => changed.service_declaration[0] ^= 1,
                    5 => changed.source.result_type = changed.source.callable,
                    6 => {
                        if let CheckedBoundaryExecution::RunCustody { operation, .. } =
                            &mut changed.execution
                        {
                            *operation = match *operation {
                                RunCustodyOperation::Split => RunCustodyOperation::Merge,
                                _ => RunCustodyOperation::Split,
                            };
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    static_source(&identities, &owner, identity, &changed).is_err(),
                    "{operation:?} fault {fault}"
                );
            }
        }
    }

    #[test]
    fn run_receiving_requires_the_complete_scalar_contract_including_empty_edges() {
        let field = |id| StructuralFieldId::new(id).unwrap();
        let fields = FieldRoles {
            storage: field(1),
            count: field(2),
            base: field(3),
            length: field(4),
            address: IntegerType::address(64).unwrap(),
        };
        for operation in [
            RunCustodyOperation::ReleaseEmpty,
            RunCustodyOperation::Split,
            RunCustodyOperation::Merge,
        ] {
            let (requires, ensures) =
                scalar_contract(operation, fields, Some((field(5), field(6)))).unwrap();
            assert!(same_clauses(&requires, requires.clone()));
            assert!(same_clauses(&ensures, ensures.clone()));
            if requires.len() > 1 {
                let mut swapped = requires.clone();
                swapped.swap(0, 1);
                assert!(
                    !same_clauses(&swapped, requires.clone()),
                    "{operation:?} swapped requirement rows"
                );
            }
            let mut swapped = ensures.clone();
            swapped.swap(0, 1);
            assert!(
                !same_clauses(&swapped, ensures.clone()),
                "{operation:?} swapped guarantee rows"
            );
            let mut missing_requirement = requires.clone();
            missing_requirement.pop();
            assert!(!same_clauses(&missing_requirement, requires));
            for ordinal in 0..ensures.len() {
                let mut forged = ensures.clone();
                forged[ordinal] = Proposition::Truth;
                assert!(
                    !same_clauses(&forged, ensures.clone()),
                    "{operation:?} guarantee {ordinal}"
                );
            }
        }
    }
}
