//! Field identity keys shared by report validation and materialization.
//!
//! A materializer selects plan entries by stable member identity when one
//! exists and by positional name otherwise; symbolic paths render through the
//! same hop vocabulary for diagnostics.

use crate::layout_plans::layout_reports::{ConventionalSumLayoutReport, LayoutPlanReport};
use crate::layout_plans::materialization::MaterializationDiagnostic;
use crate::layout_plans::symbolic_values::SymbolicFieldValue;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MaterializationFieldKey {
    Numbered(u64),
    Positional(String),
}

pub(crate) fn materialization_field_key(
    field: &str,
    member_identity: Option<u64>,
) -> MaterializationFieldKey {
    match member_identity {
        Some(identity) => MaterializationFieldKey::Numbered(identity),
        None => MaterializationFieldKey::Positional(field.to_owned()),
    }
}

pub(crate) fn validate_materialization_field_identities(
    layout: &LayoutPlanReport,
) -> Result<(), MaterializationDiagnostic> {
    let mut identity_names = std::collections::BTreeMap::new();
    let mut name_identities = std::collections::BTreeMap::new();
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        if let Some(prior_name) = identity_names.insert(key.clone(), entry.field.as_str())
            && prior_name != entry.field
        {
            return Err(MaterializationDiagnostic(format!(
                "layout field identity names both `{prior_name}` and `{}`",
                entry.field
            )));
        }
        if let Some(prior_identity) = name_identities.insert(entry.field.as_str(), key.clone())
            && prior_identity != key
        {
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` fragments do not retain the same stable identity",
                entry.field
            )));
        }
    }
    Ok(())
}

/// The same identity-pairing rule applied to a conventional sum interior: a
/// numbered schema's case or payload identity pairs with exactly one name, and
/// a case or payload name pairs with exactly one identity. Field spellings
/// stay diagnostic presentation, so the same name may spell a case and a
/// payload field under different parents.
pub(crate) fn validate_conventional_sum_materialization_identities(
    layout: &ConventionalSumLayoutReport,
) -> Result<(), MaterializationDiagnostic> {
    let mut identity_names = std::collections::BTreeMap::new();
    let mut name_identities = std::collections::BTreeMap::new();
    for case in &layout.cases {
        for (field, member_identity) in [(case.case.as_str(), case.member_identity)]
            .into_iter()
            .chain(
                case.payload_fields
                    .iter()
                    .map(|payload| (payload.field.as_str(), payload.member_identity)),
            )
        {
            let key = materialization_field_key(field, member_identity);
            if let Some(prior_name) = identity_names.insert(key.clone(), field)
                && prior_name != field
            {
                return Err(MaterializationDiagnostic(format!(
                    "sum interior identity names both `{prior_name}` and `{field}`"
                )));
            }
            if let Some(prior_identity) = name_identities.insert(field, key.clone())
                && prior_identity != key
            {
                return Err(MaterializationDiagnostic(format!(
                    "sum interior field `{field}` does not retain the same stable identity"
                )));
            }
        }
    }
    Ok(())
}

pub(crate) const fn stable_identity_suffix(member_identity: Option<u64>) -> &'static str {
    if member_identity.is_some() {
        " with the same stable identity"
    } else {
        ""
    }
}

pub(crate) fn symbolic_index_display(element_index: Option<u64>) -> String {
    match element_index {
        Some(index) => format!("[{index}]"),
        None => String::new(),
    }
}

/// The diagnostic spelling of a symbolic field path: `field`, `field[index]`,
/// `field.inner`, `field[index].inner`, `field.inner[index]`, or any deeper
/// chain of record hops such as `field[index].inner.sub[index]`. Materialized
/// writes carry it so diagnostics and relocation labels name the exact slot a
/// nested path addressed.
pub(crate) fn symbolic_path_display(symbolic: &SymbolicFieldValue) -> String {
    let mut display = format!(
        "{}{}",
        symbolic.field,
        symbolic_index_display(symbolic.element_index)
    );
    let mut segment = symbolic.inner.as_ref();
    while let Some(inner) = segment {
        display.push('.');
        display.push_str(&inner.field);
        display.push_str(&symbolic_index_display(inner.element_index));
        segment = inner.inner();
    }
    display
}

/// One segment of a spelled symbolic field path: the field name, its optional
/// stable member identity, and its optional exact element index. The outer
/// hop comes from the [`SymbolicFieldValue`]; each further hop comes from the
/// [`SymbolicFieldPathSegment`] chain it carries.
type SymbolicPathHop<'a> = (&'a str, Option<u64>, Option<u64>);

/// Flattens a spelled symbolic path into its segment list, outer hop first.
/// The chain is caller-supplied data; the depth bound is enforced during
/// derivation, not here.
pub(crate) fn symbolic_path_hops(symbolic: &SymbolicFieldValue) -> Vec<SymbolicPathHop<'_>> {
    let mut hops = vec![(
        symbolic.field.as_str(),
        symbolic.member_identity,
        symbolic.element_index,
    )];
    let mut segment = symbolic.inner.as_ref();
    while let Some(inner) = segment {
        hops.push((
            inner.field.as_str(),
            inner.member_identity,
            inner.element_index,
        ));
        segment = inner.inner();
    }
    hops
}
