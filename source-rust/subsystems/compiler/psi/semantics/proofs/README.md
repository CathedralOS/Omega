# Checking proofs in Psi

The [mathematical kernel](mathematical/mathematical.rs) checks a supplied term against its context, declarations, type and conversion budget. The Psi wire reader uses it for mathematical certificates and assumption closures. Its two dependencies provide storage for terms.

[Plain data](mathematical/plain_data.rs) builds the declarations of a data declaration whose fields all hold the data itself: the canonical trees of a W type (a raw tree paired with evidence that each child function is its constructor's own), one constructor per case and an induction principle stated over those constructors. It adds no rule and no assumption: the kernel checks every term it builds.

[Bounded proof admission](bounded/bounded.rs) checks a supplied certificate against a program obligation, its premises and source authority. It translates the covered rules to mathematical terms and asks the same kernel to judge them. [Stage 06](../../../source-to-psi/pipeline/06_lowered-program-to-checked-program/) produces candidate proofs for source obligations.

[Fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md) defines when an authorized receiver may use a producer judgment. [Mathematical values](../../../../../../wiki/spec/terminal-psi/mathematical_values.md) defines the checks on independently supplied mathematical actuals.
