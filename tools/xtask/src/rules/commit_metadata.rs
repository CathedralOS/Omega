//! Commit title lane, PR `Agent:` provenance, and compiler-lane `Gate:`/`Proof:`/`Spec:` rules.

use std::collections::BTreeSet;
use std::path::Path;

const GATE_LANES: &[&str] = &[
    "psi", "omega", "backend", "compiler", "build", "packages", "library",
];
const RELEASE_GATES: &[&str] = &[
    "RC-REPOSITORY",
    "RC-SOURCE-SEMANTICS",
    "RC-BUILD-AND-PACKAGES",
    "RC-NATIVE-MATRIX",
    "RC-DIAGNOSTICS",
    "RC-REPRESENTATIVE-PROGRAMS",
];
const SUBJECT_LIMIT: usize = 85;
const AGENT_NAMES: &[&str] = &[
    "Claude Code",
    "Codex",
    "ChatGPT",
    "Cursor",
    "Devin",
    "Antigravity",
    "Dot",
    "Human",
];

pub(super) fn lanes(agents_text: &str) -> BTreeSet<String> {
    let section = agents_text
        .split_once("## Commit naming")
        .map(|(_, s)| s)
        .unwrap_or(agents_text);
    let mut found = BTreeSet::new();
    for line in section.lines() {
        if line.starts_with("| `")
            && let Some(first_col) = line.split('|').nth(1)
        {
            let mut rest = first_col;
            while let Some(start) = rest.find('`') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('`') {
                    let lane = &after[..end];
                    if !lane.is_empty()
                        && lane
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    {
                        found.insert(lane.to_owned());
                    }
                    rest = &after[end + 1..];
                } else {
                    break;
                }
            }
        }
    }
    found
}

pub(super) fn subject_violations(subject: &str, allowed: &BTreeSet<String>) -> Vec<String> {
    let valid_lane = subject
        .split_once(": ")
        .filter(|(lane, rest)| {
            !lane.is_empty()
                && rest.chars().next().is_some_and(|c| !c.is_whitespace())
                && allowed.contains(*lane)
        })
        .is_some();
    if !valid_lane {
        let list: Vec<&str> = allowed.iter().map(String::as_str).collect();
        return vec![format!(
            "title {subject:?} does not start with a lane from AGENTS.md 'Commit naming' ({})",
            list.join(", ")
        )];
    }
    let len = subject.chars().count();
    if len > SUBJECT_LIMIT {
        return vec![format!(
            "title is {len} characters; the limit is {SUBJECT_LIMIT}"
        )];
    }
    Vec::new()
}

pub(super) fn provenance_violations(body: &str) -> Vec<String> {
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("Agent: ") {
            let name = rest.split([',', '|']).next().unwrap_or("").trim();
            if AGENT_NAMES.contains(&name) {
                return Vec::new();
            }
        }
    }
    vec![format!(
        "pull request description has no `Agent: <name>` line naming one of {} (run `cargo xtask provenance`, AGENTS.md 'Explain the change')",
        AGENT_NAMES.join(", ")
    )]
}

/// A compiler-lane PR names the release gate it serves and says what the
/// change makes work. Nothing counts rows any more, so the second field is the
/// author's own statement and is only required to be present.
pub(super) fn gate_violations(title: &str, body: &str) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str =
        "`Gate: <release-gate> | <what the change makes work>` (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Gate: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    let mut violations = Vec::new();
    if !RELEASE_GATES.contains(&fields[0]) {
        violations.push(format!(
            "`Gate:` names `{}`, not one of {}",
            fields[0],
            RELEASE_GATES.join(", ")
        ));
    }
    if !fields.get(1).is_some_and(|what| !what.is_empty()) {
        violations.push(format!("`Gate:` line is not of the form {FORM}"));
    }
    violations
}

/// A compiler-lane PR says what proved it: the tests on the change itself and
/// on the small area around it, and what they showed. The line is a statement
/// of what was run.
pub(super) fn proof_violations(title: &str, body: &str) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str = "`Proof: <tests on the change and the area around it> | <clean, or what failed and what it turned out to be>` (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Proof: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    if fields.len() >= 2 && fields.iter().all(|field| !field.is_empty()) {
        Vec::new()
    } else {
        vec![format!("`Proof:` line is not of the form {FORM}")]
    }
}

/// A compiler-lane PR names the specification page it implements, or says why
/// no page governs the change. The line is `Spec: <wiki/spec page>[#anchor][,
/// <page>...]` or `Spec: none | <reason>`; each named page must exist, so the
/// reviewer can open exactly the text the author claims to have followed.
pub(super) fn spec_violations(title: &str, body: &str, root: &Path) -> Vec<String> {
    let lane = title.split(':').next().unwrap_or("").trim();
    if !GATE_LANES.contains(&lane) {
        return Vec::new();
    }
    const FORM: &str = "`Spec: <wiki/spec/...md[#anchor]>[, <page>...]` or `Spec: none | <why no page governs this>` (AGENTS.md 'Explain the change')";
    let Some(line) = body.lines().find_map(|line| line.strip_prefix("Spec: ")) else {
        return vec![format!("{lane} pull request description has no {FORM}")];
    };
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    if fields[0] == "none" {
        return if fields.get(1).is_some_and(|why| !why.is_empty()) {
            Vec::new()
        } else {
            vec![format!("`Spec: none` needs a reason, {FORM}")]
        };
    }
    let mut violations = Vec::new();
    for page in fields[0].split(',').map(str::trim) {
        let path = page.split('#').next().unwrap_or("");
        if !path.starts_with("wiki/spec/") || !path.ends_with(".md") {
            violations.push(format!(
                "`Spec:` names `{page}`, which is not a page under wiki/spec/; {FORM}"
            ));
        } else if !root.join(path).is_file() {
            violations.push(format!("`Spec:` names `{path}`, which does not exist"));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::provenance_violations;

    #[test]
    fn chatgpt_provenance_keeps_its_actual_agent_name() {
        let body = "Summary\n\nAgent: ChatGPT | Model: unavailable | Effort: unavailable | Tokens: unavailable in (unavailable cached), unavailable out | Steps: unavailable | Time: unavailable | Steers: unavailable\n";
        assert!(provenance_violations(body).is_empty());
    }

    #[test]
    fn missing_unknown_and_near_match_agent_names_still_reject() {
        for body in [
            "Summary without an agent line\n",
            "Agent: \n",
            "Agent: Unknown | Model: unavailable\n",
            "Agent: ChatGPT-other | Model: unavailable\n",
            "Agent: chatgpt | Model: unavailable\n",
            "Summary mentions Agent: ChatGPT\n",
        ] {
            assert!(!provenance_violations(body).is_empty(), "{body:?}");
        }
    }
}
