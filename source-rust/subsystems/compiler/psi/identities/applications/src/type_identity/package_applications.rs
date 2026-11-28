//! Whole type arguments of a nominal carrier. Names and arguments use the
//! canonical identity grammar, including escaped declaration paths.

use super::framing::Reader;
use super::vocabulary::{Label, TypeForm};
use super::{DeclarationIdentitySyntax, grammar, index};

struct Application<'a> {
    declaration: &'a str,
    arguments: Vec<&'a str>,
}

fn application(text: &str) -> Option<Application<'_>> {
    let mut reader = Reader::new(text);
    let mut syntax = DeclarationIdentitySyntax::default();
    reader.expect_tag(TypeForm::Generic).ok()?;
    reader.expect(b'(').ok()?;
    let start = reader.position();
    grammar::name_atom(&mut reader, Label::Name, &mut syntax).ok()?;
    let declaration = &text[start..reader.position()];
    let mut arguments = Vec::new();
    while reader.next(b',') {
        reader.expect(b',').ok()?;
        let start = reader.position();
        index::static_argument(&mut reader, &mut syntax).ok()?;
        arguments.push(&text[start..reader.position()]);
    }
    reader.expect(b')').ok()?;
    reader.finish().ok()?;
    Some(Application {
        declaration,
        arguments,
    })
}

/// The same nominal declaration applied to the same ordered arguments after
/// substituting the package witnesses. Other type forms require their own
/// storage rule. This query does not authenticate the declaration or its fields.
/// The caller supplies complete canonical identities from the checked type rows.
pub fn substituted(public: &str, actual: &str, witnesses: &[(&str, &str)]) -> bool {
    let formals: Vec<_> = witnesses.iter().map(|&(from, _)| from).collect();
    supplied(public, actual, &formals)
        .is_some_and(|pairs| pairs.iter().all(|pair| witnesses.contains(pair)))
}

/// Read the concrete arguments corresponding to the named witnesses. Each
/// occurrence is returned so that the caller checks repeated arguments agree.
pub fn supplied<'p, 'a>(
    public: &'p str,
    actual: &'a str,
    witnesses: &[&str],
) -> Option<Vec<(&'p str, &'a str)>> {
    if witnesses
        .iter()
        .enumerate()
        .any(|(index, witness)| witnesses[..index].contains(witness))
    {
        return None;
    }
    fn walk<'p, 'a>(
        public: &'p str,
        actual: &'a str,
        witnesses: &[&str],
        found: &mut Vec<(&'p str, &'a str)>,
    ) -> Option<()> {
        let (public, actual) = (application(public)?, application(actual)?);
        if public.declaration != actual.declaration
            || public.arguments.len() != actual.arguments.len()
        {
            return None;
        }
        for (from, to) in public.arguments.into_iter().zip(actual.arguments) {
            if witnesses.contains(&from) {
                found.push((from, to));
            } else if Reader::new(from).peek::<TypeForm>() == Some(TypeForm::Generic) {
                walk(from, to, witnesses, found)?;
            } else if from != to {
                return None;
            }
        }
        Some(())
    }
    let mut found = Vec::new();
    walk(public, actual, witnesses, &mut found)?;
    Some(found)
}

/// Whether an argument in this application names this exact witness.
pub fn uses(application_text: &str, witness: &str) -> bool {
    application(application_text).is_some_and(|app| {
        app.arguments
            .iter()
            .any(|argument| *argument == witness || uses(argument, witness))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitution_keeps_the_declaration_and_every_argument_position() {
        let hidden = "named(name(hidden))";
        let file = "named(name(File))";
        let other = "named(name(Else))";
        let carrier = |name: &str, first: &str, second: &str| {
            format!("generic(name({name}),{first},{second})")
        };
        let base = carrier("Owner", other, hidden);
        let expected = carrier("Owner", other, file);
        assert!(uses(&base, hidden));
        assert!(substituted(&base, &expected, &[(hidden, file)]));
        assert!(!substituted(
            &base,
            &expected,
            &[(hidden, file), (hidden, other)]
        ));
        assert!(!substituted(
            &base,
            &carrier("Other", other, file),
            &[(hidden, file)]
        ));
        assert!(!substituted(
            &base,
            &carrier("Owner", file, other),
            &[(hidden, file)]
        ));
        assert!(!substituted(&base, &base, &[(hidden, file)]));
        let nested = carrier("Envelope", &base, hidden);
        assert!(substituted(
            &nested,
            &carrier("Envelope", &expected, file),
            &[(hidden, file)]
        ));
        assert!(!uses(&carrier("Owner", other, other), hidden));
        assert!(!substituted(
            &format!("{base}tail"),
            &expected,
            &[(hidden, file)]
        ));
    }
}
