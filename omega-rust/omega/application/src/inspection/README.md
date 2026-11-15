# Terminal inspection

[mod.rs](mod.rs) owns the reusable inspection operation;
[summary.rs](summary.rs) formats the Terminal module summary;
[commands/inspect.rs](../commands/inspect.rs) runs the CLI command and writes
its output; and [arguments/inspect.rs](../arguments/inspect.rs) parses the
request. Inspection grants no execution or native authority.

A root beside a `build.omg` is prepared through the package manager exactly as
`--check` prepares it, so declared dependency aliases resolve; a standalone root
is checked directly. Inspection stops at the checked program, before the
manager's review or trust admission.

Natural-ranked and unranked modules use ordinary verification. The old
unsigned-countdown carrier and its separate verification entry point are
retired, and a failed verification never retries with a different profile.
Natural-cycle rows name the verified components. Termination alone gives no
work ceiling; the
[logical-work contract](../../../../../wiki/spec/resources/logical_work.md)
owns quantitative analysis.

CLI regressions live in
[named_psi_inspection.rs](../../tests/named_psi_inspection.rs).

`--artifact <file>` reads an external canonical envelope through the same
public decoder that Terminal installation uses. It reports malformed semantics
before printing a module and grants no installation or execution authority.
