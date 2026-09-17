# Terminal inspection

[mod.rs](mod.rs) owns the reusable inspection operation;
[cli/inspection.rs](../cli/inspection.rs) owns its text view and
[cli/arguments/inspection.rs](../cli/arguments/inspection.rs) parses the request.
[evidence.rs](evidence.rs) verifies the selected module before trying
the existing fixed-work deriver. Inspection grants no execution or native authority.

A root beside a `build.omg` is prepared through the package manager exactly as
`--check` prepares it, so declared dependency aliases resolve; a standalone root
is checked directly. Inspection stops at the checked program without the
manager's review or trust admission.

Natural-ranked and unranked modules use ordinary verification; the legacy
unsigned-countdown carrier and its separate verification entrance are retired,
and a verification failure never retries with a different profile.
Natural-cycle rows identify the verified components.

A supported fixed-work certificate is replayed before its numeric ceiling is
printed. Unsupported analysis or an unrepresentable ceiling prints
`fixed_fuel status=unknown` with the existing deriver's diagnostic. This is a
limitation report, not proof that no finite bound exists or a complete
component/foreign-wait resource analysis. Missing identities, invalid semantics,
and failed certificate replay remain errors. The
[logical-work contract](../../../../wiki/spec/resources/logical_work.md)
owns full quantitative analysis; termination alone supplies no work ceiling.

CLI and corrupted-evidence regressions live in
[inspect_terminal.rs](../../tests/inspect_terminal.rs).
