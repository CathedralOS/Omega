//! Prepared nominal application coordinates. Construction belongs to the
//! source/canonical declaration adapter; these strings are never name lookup
//! or standalone declaration authority.

pub type ApplicationKey = (
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
    Option<String>,
);

/// A bare declaration selects every prepared receiving application of that
/// exact nominal owner. An applied choice preserves its complete telescope.
pub fn selected_application_slots(
    subject: &ApplicationKey,
    available: &[ApplicationKey],
) -> Vec<ApplicationKey> {
    if subject.2.is_some() {
        return vec![subject.clone()];
    }
    let matched = available
        .iter()
        .filter(|slot| slot.0 == subject.0 && slot.1 == subject.1)
        .cloned()
        .collect::<Vec<_>>();
    if matched.is_empty() {
        vec![subject.clone()]
    } else {
        matched
    }
}
