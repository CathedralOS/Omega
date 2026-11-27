# Checking proofs in Psi

The [mathematical kernel](mathematical/mathematical.rs) checks a supplied term against its context, declarations, type and conversion budget. The Psi wire reader uses it for mathematical certificates and assumption closures. Its two dependencies provide storage for terms.

[Plain data](mathematical/plain_data.rs) declares a data declaration, or a block of data declarations that hold each other, as one [native inductive block](mathematical/inductive.rs): a type former, a constructor per case and a recursor per member. Its fields may hold payloads, the data, another member of the block, or a function from a payload to a member. It builds case analysis and induction from those recursors. The kernel checks the block's positivity, universes and recursor type, so plain data adds no rule and no assumption.

[Bounded proof admission](bounded/bounded.rs) checks a supplied certificate against a program obligation, its premises and source authority. It translates the covered rules to mathematical terms and asks the same kernel to judge them. [Stage 06](../../../source-to-psi/pipeline/06_lowered-program-to-checked-program/) produces candidate proofs for source obligations.

[Fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md) defines when an authorized receiver may use a producer judgment. [Mathematical values](../../../../../../wiki/spec/terminal-psi/mathematical_values.md) defines the checks on independently supplied mathematical actuals.
