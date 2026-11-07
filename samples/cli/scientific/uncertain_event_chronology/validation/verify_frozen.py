"""Strict verification over the unchanged frozen independent calculation.
Keep the original reference source and its ledger intact. Its historical CLI
text comparison is not byte verification; this wrapper checks raw bytes and
source identities before checking all original and supplemental calculations.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
REPOSITORY=ROOT.parents[3]

def cases():
    return json.loads((ROOT/'data/cases.json').read_text())+json.loads((ROOT/'data/extra-cases.json').read_text())

def main():
    for ledger in ['oracle-freeze.sha256','extra-oracle-freeze.sha256']:
        for line in (ROOT/'validation'/ledger).read_text().splitlines():
            digest,relative=line.split(maxsplit=1)
            assert hashlib.sha256((REPOSITORY/relative).read_bytes()).hexdigest()==digest,relative
    spec=importlib.util.spec_from_file_location('frozen_reference',ROOT/'validation/reference.py')
    reference=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reference)
    records=cases()
    for case in records:
        folder=ROOT/'data'/case['name']
        raw=bytes.fromhex(case['input_hex'])
        assert (folder/'input.txt').read_bytes()==raw,case['name']
        code,output=reference.oracle(raw)
        assert (folder/'expected_stdout.txt').read_bytes()==output.encode('ascii'),case['name']
        assert (folder/'expected_exit.txt').read_bytes()==(str(code)+'\n').encode('ascii'),case['name']
    print(f'Frozen verification: {len(records)} exact input/stdout/exit byte records; original provenance intact')

if __name__=='__main__':main()
