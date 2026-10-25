"""Numerical qualification of actual native and retained Terminal observations.

The observer links the recorded producer libraries and uses public package,
publication and runtime APIs. Every observation must match the frozen independent
oracle, including the complete ambiguity set; exit agreement is insufficient.
"""
import argparse
from decimal import Decimal
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = ROOT.parents[3]
ORACLE_SHA = '0092fc51e689478a5e2451ee7b788cb38c065a22c71729b747d79cface3b448f'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def parse(stdout):
    rows = stdout.decode().splitlines()
    assert rows.pop(0) == 'units lattice/scale/angles/residuals 1e-12; indices/families exact'
    total = rows.pop().split()
    assert total[0] == 'hypotheses' and len(total) == 2
    hypotheses = []
    current = None
    for row in rows:
        tokens = row.split()
        if tokens[0] == 'hypothesis':
            assert current is None and len(tokens) == 4
            current = dict(kind=['P','I','F'][int(tokens[1])], lattice=Decimal(tokens[2])/Decimal('1e12'),
                           scale=Decimal(tokens[3])/Decimal('1e12'), indices=[], predicted=[], residual=[], families=[], missing=[])
        elif tokens[0] == 'peak':
            assert current is not None and len(tokens) == 4
            current['indices'].append(int(tokens[1]))
            current['predicted'].append(Decimal(tokens[2])/Decimal('1e12'))
            current['residual'].append(Decimal(tokens[3])/Decimal('1e12'))
            current['families'].append([])
        elif tokens[0] == 'family':
            assert current and current['families'] and len(tokens) == 4
            current['families'][-1].append(list(map(int,tokens[1:])))
        elif tokens[0] == 'missing':
            assert current and len(tokens) == 2
            current['missing'].append(int(tokens[1]))
        else:
            assert tokens == ['end'] and current is not None
            hypotheses.append(current)
            current = None
    assert current is None and len(hypotheses) == int(total[1])
    return hypotheses

def check(name, case, stdout, stderr, code):
    assert code == case['exit'] and not stderr, (name,code,stderr,stdout)
    if code == 64:
        assert stdout == b'error invalid input\n', (name,stdout)
        return 0.0
    observed = parse(stdout)
    key = lambda hypothesis: (hypothesis['kind'], tuple(hypothesis['indices']))
    assert len({key(item) for item in observed}) == len(observed), 'duplicate hypothesis'
    actual = {key(item):item for item in observed}
    expected = {key(item):item for item in case['hypotheses']}
    assert actual.keys() == expected.keys(), (name,'ambiguity set',actual.keys(),expected.keys())
    maximum = Decimal(0)
    for identity, reference in expected.items():
        item = actual[identity]
        assert item['families'] == reference['families'] and item['missing'] == reference['missing'], (name,identity,item)
        for label in ('lattice','scale','predicted','residual'):
            left = item[label] if isinstance(item[label],list) else [item[label]]
            right = reference[label] if isinstance(reference[label],list) else [reference[label]]
            assert len(left) == len(right)
            maximum = max(maximum, *(abs(value-Decimal(target)) for value,target in zip(left,right)))
    assert maximum <= Decimal('1e-7'), (name,maximum)
    return float(maximum)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True, type=Path)
    parser.add_argument('--route', choices=['native','terminal','all'], default='all')
    parser.add_argument('--output', type=Path, default=ROOT/'build/validation')
    options = parser.parse_args()
    compiler = options.omega.resolve()
    output = options.output.resolve()
    output.mkdir(parents=True,exist_ok=True)
    assert digest(ROOT/'validation/oracle.json') == ORACLE_SHA, 'frozen oracle changed'
    oracle = json.loads((ROOT/'validation/oracle.json').read_text())
    for name,case in oracle['cases'].items():
        assert digest(ROOT/'data'/f'{name}.txt') == case['input_sha256'], name
    libraries = {}
    directories = set()
    for name in ('omega', 'omega_build','compiler', 'terminal_runtime','package_manager','terminal_interpreter','target','bounded_process'):
        paths = list((compiler.parent/'build').glob(f'{name.replace("_","-")}/*/out/lib{name}-*.rlib'))
        assert paths, (name,'missing recorded producer library',compiler)
        selected = max(paths,key=lambda path:path.stat().st_mtime_ns)
        libraries[name] = selected
    for path in (compiler.parent/'build').glob('*/*/out'):
        directories.add(path)
    command = ['rustc','--edition=2024','-C','opt-level=1',str(ROOT/'validation/observe.rs'),'-o',str(output/'observer')]
    for name,path in libraries.items():
        command += ['--extern',f'{name}={path}']
        metadata = path.with_suffix('.rmeta')
        if metadata.exists():
            command += ['--extern',f'{name}={metadata}']
    for path in sorted(directories):
        command += ['-L',f'dependency={path}']
    identity_paths = [compiler, *libraries.values(), *(path.with_suffix('.rmeta') for path in libraries.values() if path.with_suffix('.rmeta').exists()), ROOT/'validation/observe.rs', ROOT/'validation/validate.py', ROOT/'validation/oracle.json']
    identity_paths += [path for path in ROOT.rglob('*.omg') if 'build' not in path.relative_to(ROOT).parts]
    identity_paths += list((REPOSITORY/'source/library').rglob('*.omg'))
    identities = {str(path):digest(path) for path in identity_paths}
    (output/'identities.before.json').write_text(json.dumps(identities,indent=2)+'\n')
    built = subprocess.run(command,capture_output=True,timeout=180)
    (output/'observer-build.stdout').write_bytes(built.stdout)
    (output/'observer-build.stderr').write_bytes(built.stderr)
    assert built.returncode == 0, built.stderr.decode(errors='replace')
    started = time.monotonic()
    command = [str(output/'observer'),str(ROOT),str(output),'100000000','30',*oracle['cases']]
    with (output/'capture.log').open('wb') as stream:
        captured = subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=1200)
    assert captured.returncode == 0, (output/'capture.log').read_text()
    assert identities == {str(path):digest(path) for path in identity_paths}, 'producer/source changed during qualification'
    engines = ['native','terminal'] if options.route == 'all' else [options.route]
    results = []
    for name,case in oracle['cases'].items():
        for engine in engines:
            error = check(name,case,(output/f'{name}.{engine}.stdout').read_bytes(),
                (output/f'{name}.{engine}.stderr').read_bytes(),int((output/f'{name}.{engine}.exit').read_text()))
            results.append(dict(case=name,engine=engine,exit=case['exit'],max_numeric_error=error))
    invalid = subprocess.run([str(compiler),'--check',str(ROOT/'controls/reflection_bound/main.omg')],input=b'',capture_output=True,timeout=120)
    (output/'source-invalid.stdout').write_bytes(invalid.stdout)
    (output/'source-invalid.stderr').write_bytes(invalid.stderr)
    assert invalid.returncode == 1 and b'index `8`' in invalid.stderr and b'length 8' in invalid.stderr, invalid.stderr.decode(errors='replace')
    record = dict(oracle_sha256=ORACLE_SHA, compiler_sha256=digest(compiler),
        producer_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
        source_diff_sha256=hashlib.sha256(subprocess.check_output(['git','diff'],cwd=REPOSITORY)).hexdigest(),
        seconds=time.monotonic()-started, results=results, source_invalid_exit=invalid.returncode,
        observer_sha256=digest(output/'observer'))
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record,indent=2))

if __name__ == '__main__':
    main()
