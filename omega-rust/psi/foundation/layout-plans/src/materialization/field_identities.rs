//! Field identity keys shared by report validation and materialization.
//!
//! A materializer selects plan entries by stable member identity when one
//! exists and by positional name otherwise; symbolic paths render through the
//! same hop vocabulary for diagnostics.

use crate::layout_reports::{
    ConventionalSumLayoutReport, LayoutFieldEntryReport, LayoutPlanReport,
};
use crate::materialization::MaterializationDiagnostic;
use crate::symbolic_values::SymbolicFieldValue;

use std::collections::{BTreeMap, BTreeSet};

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

/// The validated plan's entries indexed by the materialization key that
/// selects them: one entry for a scalar placement, and one entry per element
/// or fragment for a repeated or fragmented field. Materializers, symbolic
/// derivation, and interior-carrier preparation all read this index rather
/// than rebuilding it per operation.
pub(crate) fn planned_entries_by_key(
    layout: &LayoutPlanReport,
) -> BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> {
    let mut planned: BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> =
        BTreeMap::new();
    for entry in &layout.entries {
        planned
            .entry(materialization_field_key(
                &entry.field,
                entry.member_identity,
            ))
            .or_default()
            .push(entry);
    }
    planned
}

/// One caller-supplied field beside the spelling and compiler-retained stable
/// identity that select its placements.
struct SuppliedField<'fields, Field> {
    value: &'fields Field,
    name: &'fields str,
    member_identity: Option<u64>,
}

/// One caller-supplied field collection indexed by stable key, beside the
/// spellings it supplied. `kind` and the duplicate verb re-spell the caller's
/// vocabulary in the two duplicate diagnostics; field spellings stay
/// diagnostic presentation, so a rename never changes selection.
pub(crate) struct SuppliedFields<'fields, Field> {
    by_key: BTreeMap<MaterializationFieldKey, SuppliedField<'fields, Field>>,
    by_name: BTreeMap<&'fields str, &'fields Field>,
}

impl<'fields, Field> SuppliedFields<'fields, Field> {
    /// The supplied field selecting `key`, if any.
    pub(crate) fn value(&self, key: &MaterializationFieldKey) -> Option<&'fields Field> {
        self.by_key.get(key).map(|supplied| supplied.value)
    }

    /// The supplied field spelled `name`, if any. A numbered field still
    /// carries its presentation spelling beside its stable identity.
    pub(crate) fn named(&self, name: &str) -> Option<&'fields Field> {
        self.by_name.get(name).copied()
    }

    /// The supplied fields by stable key, in key order.
    pub(crate) fn fields_by_key(
        &self,
    ) -> impl Iterator<Item = (&MaterializationFieldKey, &'fields Field)> {
        self.by_key
            .iter()
            .map(|(key, supplied)| (key, supplied.value))
    }

    /// The supplied spellings, in name order.
    pub(crate) fn names(&self) -> impl Iterator<Item = &'fields str> {
        self.by_name.keys().copied()
    }
}

/// Indexes one supplied field collection by the key that selects its
/// placements: a repeated spelling rejects first, then a repeated stable
/// identity, because either leaves two supplied fields selecting one
/// placement. The name-keyed side stays available for the callers whose
/// supplied collection carries no identity dimension.
pub(crate) fn index_supplied_fields<'fields, Field>(
    fields: &'fields [Field],
    kind: &str,
    duplicate_verb: &str,
    supplied: impl Fn(&'fields Field) -> (&'fields str, Option<u64>),
) -> Result<SuppliedFields<'fields, Field>, MaterializationDiagnostic> {
    let mut indexed = SuppliedFields {
        by_key: BTreeMap::new(),
        by_name: BTreeMap::new(),
    };
    for field in fields {
        let (name, member_identity) = supplied(field);
        if indexed.by_name.insert(name, field).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "{kind} field `{name}` is {duplicate_verb} more than once"
            )));
        }
        let key = materialization_field_key(name, member_identity);
        let indexed_field = SuppliedField {
            value: field,
            name,
            member_identity,
        };
        if indexed.by_key.insert(key, indexed_field).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "{kind} field `{name}` repeats stable member identity #{}",
                member_identity.expect("only numbered fields can collide after name validation")
            )));
        }
    }
    Ok(indexed)
}

/// The complete-field-set pairing rule between a validated plan and one
/// supplied field collection: every placement the plan retains names one
/// supplied field, and every supplied field names one retained placement.
/// Either direction left unpaired would let a materializer stage from a plan
/// neither side describes. `missing_noun` and `extra_noun` spell the two
/// directions in the caller's vocabulary.
pub(crate) fn validate_supplied_field_coverage<Field>(
    layout: &LayoutPlanReport,
    supplied: &SuppliedFields<'_, Field>,
    missing_noun: &str,
    extra_noun: &str,
) -> Result<(), MaterializationDiagnostic> {
    if let Some(entry) = layout.entries.iter().find(|entry| {
        !supplied.by_key.contains_key(&materialization_field_key(
            &entry.field,
            entry.member_identity,
        ))
    }) {
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no {missing_noun}{suffix}",
            entry.field
        )));
    }
    let planned = layout
        .entries
        .iter()
        .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
        .collect::<BTreeSet<_>>();
    if let Some(field) = supplied
        .by_key
        .iter()
        .find_map(|(key, field)| (!planned.contains(key)).then_some(field))
    {
        let suffix = stable_identity_suffix(field.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "{extra_noun} `{}` has no entry in the validated layout plan{suffix}",
            field.name
        )));
    }
    Ok(())
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
