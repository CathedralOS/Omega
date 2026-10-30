from pathlib import Path
root = Path.cwd()
owner = root/'omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment'
def instrument(path, function, label):
    source = path.read_text(encoding='utf-8')
    start = source.index('fn '+function+'(')
    body = source.index(' {\n', start) + 3
    source = source[:body] + f'        let _profile = phase_profile::guard("05.remaining.{label}");\n' + source[body:]
    path.write_text(source, encoding='utf-8', newline='\n')
selected=owner/'structural_judgment/selected_contracts.rs'
for function,label in [('selected_contract_normal_form','normal_form'),('specification_application_equations','equations'),('recorded_body_boolean','body_boolean')]:
    instrument(selected,function,label)
instrument(owner/'structural_judgment.rs','unfold_application','unfold')
instrument(owner/'structural_judgment/resolution_queries.rs','resolving_application','active_question')
source=selected.read_text()
start=source.index('    fn declared_guarantees(')
body=source.index('        let program = self.program;',start)
insert='''        let has_equations = owner_contracts(self.program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
            .any(|contract| self.program.proof_facts.span_or_empty(contract.facts).iter().any(|fact| matches!(fact, ProofFact::Expression(_))));
        let _profile = phase_profile::guard(if has_equations { "05.remaining.guarantees.with_ensures" } else { "05.remaining.guarantees.without_ensures" });
'''
source=source[:body]+insert+source[body:]
start=source.index('        for contract in owner_contracts(program, owner)',body)
end=source.index('        let mut equations = Vec::new();',start)
source=source[:start]+'''        {
        let _requirements = phase_profile::guard(if has_equations { "05.remaining.requires.with_ensures" } else { "05.remaining.requires.without_ensures" });
'''+source[start:end]+'        }\n'+source[end:]
selected.write_text(source,encoding='utf-8',newline='\n')
