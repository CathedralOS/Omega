//! Provider supply for the boundary requirements a program entry reaches.
//!
//! Selection chooses among declared candidates, and a missing candidate
//! rejects (`wiki/spec/build/provider_selection.md`).
//! A requirement the entry calls directly, `Requirement::operation(..)`, has
//! no source carrier that could name its boundary early, so its demand is read
//! here from this target's realized plan. Psi's invocation facts union every
//! member of a target family; the realized plan reaches only the member this
//! target selected, so another target's raw family is never demanded.
//!
//! The walk is the closure review's own (`reachability`). This check names the
//! entry's unselected requirements before settlement reports the same omission
//! as a missing import, mechanism or provider row. A selected provider's
//! nested calls are closed by provider planning; the review still refuses any
//! reachable requirement without a row.

use std::borrow::Cow;

use ::diagnostics::Diagnostic;

use super::diagnostics::realization_error;
use super::terminal_authority_review::reachability::AuthorityGraph;

pub(super) fn require_reached_providers(
    input: &crate::VerifiedPsiOptimizationInput,
    target_profile: target::TargetProfile,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let plan = input.plan();
    let reached = AuthorityGraph::new(input, target_profile)
        .and_then(|graph| graph.reachable_authority_edges(plan.entry))
        .map_err(|error| realization_error("provider supply", error))?;
    let diagnostics = reached
        .boundaries
        .iter()
        .filter_map(|boundary| {
            // An undeclared boundary is the closure review's refusal.
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)?;
            // Private memory operations are not supplied by provider selection.
            // Their operation owner joins the installed profile call and keeps
            // the native-execution fence until backing realization exists.
            if declaration.declaration.as_ref()
                .and_then(|declaration| declaration.execution.placement_operation())
                .is_some() {
                return None;
            }
            let requirement = plan.identities.text(declaration.identity)?;
            (selected
                .selected_requirement_row(requirement)
                .is_none()
                && !selected.assumed_bindings().iter().any(|row| row.required == requirement)
            ).then(|| {
                    Diagnostic::error(format!(
                        "no provider is selected for boundary requirement `{}`, which the program entry reaches on target `{}`",
                        requirement_path(requirement),
                        target_profile.target_name(),
                    ))
                })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The authored path inside a canonical callable identity, for this
/// diagnostic only: `named-callable(path(Gui::window_create),parameters(..))`
/// renders as `Gui::window_create`. A callable owned by a managed package or
/// the toolchain carries that path inside its nominal owner,
/// `path(nominal\(package-owner\(32:..\)\,path\(Gui::window_create\)\))`, and
/// renders as the same authored path: the message names the requirement the
/// author wrote, never the owner digest. No join reads it; an identity without
/// the head renders whole.
fn requirement_path(identity: &str) -> Cow<'_, str> {
    const HEAD: &str = "named-callable(path(";
    let Some(start) = identity.find(HEAD).map(|index| index + HEAD.len()) else {
        return Cow::Borrowed(identity);
    };
    let Some((path, _)) = atom_payload(&identity[start..]) else {
        return Cow::Borrowed(identity);
    };
    // Qualification is `nominal(<owner>,path(<authored>))`. No owner atom
    // holds a comma or an escape, so the first `,path(` opens the authored path.
    let authored = path
        .strip_prefix("nominal(")
        .and_then(|qualified| qualified.split_once(",path("))
        .and_then(|(_, authored)| atom_payload(authored))
        .map(|(authored, _)| authored.into_owned());
    authored.map_or(path, Cow::Owned)
}

/// The unescaped payload of the atom that begins `text`, up to its first
/// unescaped `)`, with the length consumed including that delimiter. Atoms
/// escape `\`, `(`, `)` and `,` with a backslash, so every unescaped `)` ends
/// one. `None` when the atom never closes.
fn atom_payload(text: &str) -> Option<(Cow<'_, str>, usize)> {
    let mut characters = text.char_indices();
    while let Some((offset, character)) = characters.next() {
        match character {
            '\\' => {
                characters.next()?;
            }
            ')' => {
                let raw = &text[..offset];
                let payload = if raw.contains('\\') {
                    let mut unescaped = String::with_capacity(raw.len());
                    let mut raw_characters = raw.chars();
                    while let Some(character) = raw_characters.next() {
                        unescaped.push(if character == '\\' {
                            raw_characters.next().unwrap_or(character)
                        } else {
                            character
                        });
                    }
                    Cow::Owned(unescaped)
                } else {
                    Cow::Borrowed(raw)
                };
                return Some((payload, offset + 1));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::requirement_path;

    const OWNER: &str =
        "package-owner\\(32:31ed5a8cdac79074acce8823cec1b3e85e1a2a4d42afa87343d6efc3220c7a2b\\)";

    #[test]
    fn requirement_path_names_the_authored_callable() {
        assert_eq!(
            requirement_path(
                "named-callable(path(Acknowledgement::ack),parameters(parameter\\(x\\)),result-dispatch())"
            ),
            "Acknowledgement::ack"
        );
        assert_eq!(
            requirement_path(
                "closed-requirement(template(named-callable(path(Parent::read),parameters())))"
            ),
            "Parent::read"
        );
        assert_eq!(requirement_path("opaque"), "opaque");
    }

    #[test]
    fn requirement_path_looks_through_the_nominal_owner() {
        // The identity a managed package's generic native requirement carries.
        let qualified = format!(
            "named-callable(path(nominal\\({OWNER}\\,path\\(CText::length\\)\\)),parameters(parameter\\(x\\)),result-dispatch())"
        );
        assert_eq!(requirement_path(&qualified), "CText::length");
        // The toolchain and unresolved owners carry no digest atom at all.
        for owner in ["toolchain-owner", "unresolved-owner"] {
            let qualified = format!(
                "named-callable(path(nominal\\({owner}\\,path\\(emit\\)\\)),parameters(),result-dispatch())"
            );
            assert_eq!(requirement_path(&qualified), "emit");
        }
    }

    #[test]
    fn requirement_path_renders_what_it_cannot_unwrap_as_written() {
        // An atom that never closes is not a path at all: the identity is whole.
        assert_eq!(
            requirement_path("named-callable(path(Gui::window_create"),
            "named-callable(path(Gui::window_create"
        );
        // A nominal wrapper with no authored path inside keeps its path atom.
        assert_eq!(
            requirement_path("named-callable(path(nominal\\(toolchain-owner\\)),parameters())"),
            "nominal(toolchain-owner)"
        );
    }
}
