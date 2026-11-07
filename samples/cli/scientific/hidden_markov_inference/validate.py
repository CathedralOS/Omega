"""Validate native and independently captured Terminal stdout against frozen data."""
import argparse, hashlib, json, math, subprocess, time
from pathlib import Path
if not __debug__:
    raise RuntimeError('validation requires assertions; run Python without -O or PYTHONOPTIMIZE')
ROOT=Path(__file__).resolve().parent

def check(output,case):
    lines=output.decode().splitlines()
    if case['status']!='OK':
        assert lines==[case['status']], lines
        return
    assert lines[0]=='OK trillionths',lines
    assert len(lines)==len(case['posterior'])+3,lines
    log_fields=lines[1].split(); assert len(log_fields)==2 and log_fields[0]=='LOG',log_fields
    actual_log=int(log_fields[1])/1e12
    assert abs(actual_log-float(case['log_likelihood']))<=1e-9,(actual_log,case['log_likelihood'])
    for index,(line,row) in enumerate(zip(lines[2:-1],case['posterior'])):
        fields=line.split();assert fields[0]==f'POST{index}',fields
        values=[int(value)/1e12 for value in fields[1:]]
        assert len(values)==2 and abs(sum(values)-1)<=2e-9
        for actual,expected in zip(values,row): assert abs(actual-float(expected))<=1e-9,(index,actual,expected)
    assert lines[-1]=='PATH '+''.join(map(str,case['path'])),lines[-1]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--native',required=True);parser.add_argument('--terminal',required=True);parser.add_argument('--producer',required=True)
    args=parser.parse_args();terminal=Path(args.terminal);oracle=ROOT/'expected.json'
    cases=json.loads(oracle.read_text())['cases'];out=ROOT/'build/validation';out.mkdir(parents=True,exist_ok=True)
    sources=dict(line.split('\t') for line in (terminal/'sources.tsv').read_text().splitlines())
    for name,digest in sources.items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,('stale source',name)
    assert set(sources)=={'main.omg','build.omg','domain/inference.omg','domain/viterbi.omg','io/trace.omg'}
    assert hashlib.sha256((terminal/'application.terminal').read_bytes()).hexdigest()==(terminal/'terminal.sha256').read_text()
    assert hashlib.sha256(Path(args.native).read_bytes()).hexdigest()==(terminal/'native.sha256').read_text()
    assert hashlib.sha256(Path(args.producer).read_bytes()).hexdigest()==(terminal/'producer.sha256').read_text()
    rows={fields[0]:fields[1:] for line in (terminal/'records.tsv').read_text().splitlines()[1:] for fields in [line.split('\t')]}
    assert set(rows)==set(cases)
    records=[]
    for name,case in cases.items():
        data=(ROOT/'data'/f'{name}.stdin').read_bytes()
        assert hashlib.sha256(data).hexdigest()==case['input_sha256']
        start=time.monotonic();result=subprocess.run([args.native],input=data,capture_output=True,timeout=30)
        (out/f'{name}.native.stdout').write_bytes(result.stdout);(out/f'{name}.native.stderr').write_bytes(result.stderr)
        assert result.returncode==case['exit'],(name,result.returncode)
        check(result.stdout,case)
        row=rows[name];assert len(row)==4 and int(row[0])==case['exit'] and row[1]==case['input_sha256'],(name,row)
        for suffix,digest in zip(('stdout','stderr'),row[2:]):assert hashlib.sha256((terminal/f'{name}.{suffix}').read_bytes()).hexdigest()==digest
        check((terminal/f'{name}.stdout').read_bytes(),case)
        records.append({'case':name,'exit':result.returncode,'seconds':time.monotonic()-start})
        print('matched native and captured Terminal',name,flush=True)
    (out/'receipt.json').write_text(json.dumps({'oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(Path(args.native).read_bytes()).hexdigest(),'terminal_sha256':(terminal/'terminal.sha256').read_text(),'producer_sha256':(terminal/'producer.sha256').read_text(),'sources':sources,'cases':records},indent=2)+'\n')
if __name__=='__main__':main()
