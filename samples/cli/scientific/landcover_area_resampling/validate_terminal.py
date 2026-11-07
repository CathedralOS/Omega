"""Compare complete native and Terminal streams through the documented corpus route.

Scratch fixtures contain the unchanged application, with only their Build path
adapted. Freeze oracles before execution; no golden or checked-tree fallback.
"""
import argparse, hashlib, json, os, shutil, subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('runner',type=Path,help='verified corpus_runner executable')
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parent
repository=root.parents[3]
output=args.output.resolve()
output.mkdir(parents=True,exist_ok=True)
fixtures=repository/'tests/omega/run/new047_landcover_area_resampling_matrix'
assert not fixtures.exists(), 'refuse to replace an existing fixture tree'
cases=[(f'fixture_{mode}',(root/'data'/f'input_{mode}.txt').read_bytes(),0,
        (root/'data'/f'expected_{mode}.txt').read_bytes()) for mode in range(1,7)]
invalid=b'LANDCOVER exact planar area; enter fixture 1..6 (7..9 invalid)\nINVALID: bounded fixture input or model admission\n'
cases += [(name,payload,64,invalid) for name,payload in [
    ('width',b'7\n'),('class',b'8\n'),('extent',b'9\n'),('unknown',b'0\n'),
    ('text',b'x\n'),('extra',b'12\n'),('eof',b''),('unterminated',b'1'),('full',b'1234\n')]]
sources=['main.omg','landcover/geometry.omg','landcover/resampling.omg','fixtures/catalog.omg']
source_hashes={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in sources}
freeze=dict(source_sha256=source_hashes,cases=[dict(name=name,input_hex=payload.hex(),
    exit=status,stdout_sha256=hashlib.sha256(expected).hexdigest(),stderr_sha256=hashlib.sha256(b'').hexdigest())
    for name,payload,status,expected in cases],absolute_tolerance=0,relative_tolerance=0)
(output/'oracle-freeze.json').write_text(json.dumps(freeze,indent=2)+'\n')
try:
    for name,payload,status,expected in cases:
        directory=fixtures/name
        directory.mkdir(parents=True)
        for filename in sources:
            destination=directory/filename
            destination.parent.mkdir(parents=True,exist_ok=True)
            shutil.copy2(root/filename,destination)
            assert hashlib.sha256(destination.read_bytes()).hexdigest()==source_hashes[filename]
        (directory/'build.omg').write_text('machine build(builder: &mut Build) {\n'
            ' Build::application(builder, "landcover_area_resampling");\n'
            ' Build::depend(builder, Source::Path { location: "../../../../../source/library/std" });\n'
            ' Build::bind(builder, linux_x86_64::ProgramEntry, Main::main);\n}\n')
        (directory/'input.txt').write_bytes(payload)
        (directory/'expected_stdout.txt').write_bytes(expected)
        (directory/'expected_stderr.txt').write_bytes(b'')
        (directory/'expected_native_exit.txt').write_text(str(status)+'\n')
    command=['python3','tools/corpus_gate.py','--runner',str(args.runner.resolve()),'--both',
        '--golden','tests/omega/corpus_outcomes.txt','--filter','run/new047_landcover_area_resampling_matrix/',
        '--json',str(output/'verdict.json'),'--write-moves',str(output/'moves.json')]
    (output/'command.json').write_text(json.dumps(command,indent=2)+'\n')
    environment=dict(os.environ,OMEGA_CORPUS_JOBS='1',OMEGA_CORPUS_FIXTURE_SECS='300')
    with (output/'gate.stdout').open('wb') as stdout, (output/'gate.stderr').open('wb') as stderr:
        result=subprocess.run(command,cwd=repository,env=environment,stdout=stdout,stderr=stderr)
    verdict=json.loads((output/'verdict.json').read_text())
    assert not verdict.get('oracle_failures'),verdict.get('oracle_failures')
    roster={'run/new047_landcover_area_resampling_matrix/'+name:status for name,_,status,_ in cases}
    rows=verdict['observations']
    assert len(rows)==len(roster)==15
    assert {row['fixture'] for row in rows}==set(roster)
    for row in rows:
        assert row['status']=='built',row
        for prefix in ('','interpreter-'):
            facts=row['facts']
            assert facts.get(prefix+'exit')==str(roster[row['fixture']]),row
            assert facts.get(prefix+'stdout')=='match',row
            assert facts.get(prefix+'stderr')=='match',row
        assert row['facts'].get('interpreter')=='agrees',row
    # Scratch rows are absent from the checked-in golden. Accept its structural
    # addition (exit 1) only after all authored stream and exit promises above.
    assert result.returncode in (0,1),result.returncode
    (output/'results.json').write_text(json.dumps(dict(gate_exit=result.returncode,
        oracle_freeze_sha256=hashlib.sha256((output/'oracle-freeze.json').read_bytes()).hexdigest(),
        observations=rows),indent=2)+'\n')
    print('PASS: fifteen complete native and Terminal stdout/stderr/exit oracles')
finally:
    shutil.rmtree(fixtures)
