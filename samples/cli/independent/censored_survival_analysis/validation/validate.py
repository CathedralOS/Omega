"""Verify frozen inputs, exact counts/statuses and numerical captures <= 1e-9.
Runs either the retained native image or the public accepted-artifact Terminal
sponsor. CLI exit comparison is separate and cannot substitute for captures.
"""
import argparse, hashlib, json, subprocess, time
from fractions import Fraction
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def frozen():
    for line in ((ROOT/'validation/oracle-freeze.sha256').read_text()+(ROOT/'validation/extra-freeze.sha256').read_text()).splitlines():
        digest,name=line.split('  ',1)
        assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name

def compare(raw, expected):
    if 'error' in expected:
        assert raw==expected['error'].encode(),raw
        return
    lines=iter(raw.decode().splitlines())
    def close(actual, exact):
        assert abs(float(actual)-float(Fraction(exact)))<=1e-9,(actual,exact)
    for table in expected['tables']:
        assert next(lines)==f"TABLE {table['group']}"
        for row in table['rows']:
            fields=next(lines).split(); assert fields[0]=='ROW' and len(fields)==8,fields
            close(fields[1],row[0]); assert list(map(int,fields[2:5]))==row[1:4]
            close(fields[5],row[4]); close(fields[6],row[5]); assert int(fields[7])==row[6]
        median=next(lines).split(); assert median[0]=='MEDIAN' and len(median)==4
        assert int(median[1])==int(table['median'] is not None)
        close(median[2],table['median'] or '0'); assert int(median[3])==len(table['rows'])
    fields=next(lines).split(); assert fields[0]=='LOGRANK' and len(fields)==6
    for actual,exact in zip(fields[1:5],expected['logrank']): close(actual,exact or '0')
    assert int(fields[5])==int(expected['logrank'][3] is not None)
    assert next(lines,None) is None

def main():
    parser=argparse.ArgumentParser(); routes=parser.add_mutually_exclusive_group(required=True); routes.add_argument('--native',type=Path); routes.add_argument('--terminal',type=Path); parser.add_argument('--captures',type=Path,required=True)
    arguments=parser.parse_args(); frozen(); arguments.captures.mkdir(parents=True,exist_ok=True)
    expected=json.loads((ROOT/'validation/expected.json').read_text()); expected.update(json.loads((ROOT/'validation/extra_expected.json').read_text())); records=[]
    if arguments.terminal:
        completed=subprocess.run([arguments.terminal.resolve(),ROOT,ROOT/'validation/all_cases.txt',arguments.captures.resolve()],input=b'',capture_output=True,timeout=1200)
        (arguments.captures/'sponsor.stdout').write_bytes(completed.stdout); (arguments.captures/'sponsor.stderr').write_bytes(completed.stderr)
        assert completed.returncode==0,completed.stderr.decode()
    for name,oracle in expected.items():
        started=time.monotonic(); folder=ROOT/'data'/name; code=int((folder/'expected_exit.txt').read_text())
        if arguments.native:
            completed=subprocess.run([arguments.native.resolve()],input=(folder/'input.txt').read_bytes(),capture_output=True,timeout=30)
            (arguments.captures/(name+'.stdout')).write_bytes(completed.stdout); (arguments.captures/(name+'.stderr')).write_bytes(completed.stderr)
            assert completed.returncode==code,(name,completed.returncode)
        else:
            row=next(r for r in (arguments.captures/'records.tsv').read_text().splitlines() if r.startswith(name+'\t')).split('\t')
            assert row[1:3]==['PASS',f'Some({code})'],row
        raw=(arguments.captures/(name+'.stdout')).read_bytes(); assert (arguments.captures/(name+'.stderr')).read_bytes()==b''
        compare(raw,oracle); records.append({'name':name,'exit':code,'passed':True,'seconds':time.monotonic()-started,'stdout_sha256':hashlib.sha256(raw).hexdigest()}); print(name,'PASS',flush=True)
    (arguments.captures/'validation.json').write_text(json.dumps({'route':'native' if arguments.native else 'captured Terminal','tolerance':1e-9,'cases':records},indent=2)+'\n')
if __name__=='__main__': main()
