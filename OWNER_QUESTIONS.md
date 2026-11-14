# Owner Questions

Only the owner answers an entry. The owner's word in a session is the
ruling: write it into the specification, remove the entry, and build it; that
is what the owner asked for. Otherwise agents add questions and may research
existing ones, but do not settle a question themselves.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap construction follows the owner's [variant mandate](bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### Q1 - May a qualified operator be declared outside the domain's module?

Decision name: `unresolved-qualified-operator-home`.

May an operator be declared at the root, outside the module that declares
its domain, under a qualified spelling `Carrier::Leaf::op`? The shape is
`operator u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance`.
Or must that operator be declared inside the domain's own module, and a
root-level qualified name be rejected? Pull request #2965 (merged as
`f1b00a19b1b`, "psi: select domains through the shared source scope") homes
a reaching qualified name on the domain. Before it, the operator stayed a
root item and `a + b` on those values did not find it. Closed pull request
#2957 had drafted this under the same decision name and did not land.

**Audit.** The root spelling is implemented, and no authored source uses
it. `normalize_domain_operator_homes` moves a top-level operator into the
domain whose name reaches from the declaration.
`domain_exposure_explicit_owner_uses_the_same_scope`
(`omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_domain_exposure_tests.rs:268`)
puts `domain u64::Mark` in `module units` and a synthesized path
`u64::Mark::operation` on the importing file, and the operator joins that
domain. The test builds the operator record in Rust. It does not parse an
Omega declaration of that path. The parser's message at
`omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/declarations.rs:1305`
requires an operator to be a top-level declaration written after the
domain declaration. The domain's brace body is retired. Which module
holds that top-level declaration is the open question.

Authored programs that do declare a qualified operator put it in the
domain's own module. `samples/cli/probes/module_domain_path/units.omg`
writes `operator + u64::Distance::add` inside `module units`, beside
`domain u64::Distance`. The same file holds both the domain and the
operator in `samples/cli/probes/domain_operator_roster/main.omg`,
`samples/cli/probes/additive_domain_requires/main.omg`,
`samples/cli/probes/domain_erasure_established/main.omg`,
`source/library/core/units.omg` (`machine + Quantity::add_i64_meters`
beside `T::Quantity<U>`), and
`tests/fixtures/pass/expressions/declared_operator_match_result/main.omg`
(`machine + Wrapped::add` beside `data Wrapped`). No sample, kept fixture,
specification example, or language-guide example declares
`Carrier::Leaf::op` in a file that is not the domain's module.

**Product requirement.** There is none. An operator written in the
domain's module already supplies `a + b`. No sample, library file, or
fixture needs the operator text to live in another module. The homing
test is not a product requirement.

**Clauses checked and found silent.** These were read for a rule that
either admits a root qualified operator or rejects it. None does.

- `wiki/spec/language/expressions.md:145-161`. An operator binding is a
  named machine, free or module, trait, or implementation qualified.
  Line 160 says there is no separate `operator` introducer. The section
  does not say which module may hold a domain-qualified declaration.
- `wiki/spec/language/expressions.md:299-322`. A qualified declaration
  such as `Domain::operation` establishes its semantic home. An
  unqualified name may establish the same home through one operand-domain
  constraint. Line 322 leaves open-family syntax unsettled; that is a
  different decision. The section does not require the text to sit inside
  the domain's module.
- `wiki/spec/language/expressions.md:324-328`. Only the semantic-home
  owner publishes a closed family's direct token bindings, and a
  declaration in an unauthorized package is rejected. That names the
  owning package.
- `wiki/spec/language/domains.md:23-29`. Operations are separate machines,
  and a domain declaration ends with `;`. The section does not name the
  file that declares those machines.
- `wiki/spec/language/domains.md:236-239`. Importing a domain does not
  implicitly change operators. It does not say whether another file may
  declare one.
- `wiki/spec/language/domains.md:884-896`. The example
  `machine + Km::add_wrapping` is written with the domains it serves, with
  no module boundary.
- `wiki/spec/language/modules.md:52-75`. What a module import exposes.
  Silent on where an operator is declared.
- `wiki/spec/language/modules.md:77-94`. A short carrier-qualified path
  selects a domain when the carrier resolves and the domain is exposed.
  It does not accept or reject a root operator written with that path.
- `wiki/spec/language/modules.md:140-142`. A machine head cannot attach
  to a data carrier. That is data attachment.
- `wiki/language_guide/chapter_8_domains.md:111-123`. A domain gets
  operators by declaring `machine + Degrees::add` beside
  `domain u32::Degrees`. The section shows no other module.
- `wiki/language_guide/chapter_5_expressions_evaluation.md:56-71`.
  `machine + Vec2::add` is declared by the type's owner, so an unrelated
  import cannot change `+`. The example is a data type, and the sentence
  names the owner.
- `wiki/language_guide/chapter_15_modules_imports_visibility.md:19-42`.
  Modules and imports. No operator rule.

**Both forms.** The inside form is copied from
`samples/cli/probes/module_domain_path/units.omg`. This note did not
compile it again. The root form is that same operator moved to a file
that only imports `units`. No file in the repository contains it, and
this note did not run it.

Inside the domain's module:

```omega
module units;

public domain u64::Distance requires self > 0;

public operator + u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance;
```

At the root of another file:

```omega
use units;

public operator + u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance;
```

**Question.** Is the root file allowed to declare that operator, or must
the declaration stay in the domain's module and the root spelling be
rejected?

**Until answered.** A qualified name that reaches the domain is homed on
that domain, including from the importing file. The clauses above do not
adopt that behavior.
