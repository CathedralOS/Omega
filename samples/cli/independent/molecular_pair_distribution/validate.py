"""Check actual native or captured Terminal values; frozen oracles are read-only."""
import argparse,hashlib,json,subprocess,time
from decimal import Decimal
from pathlib import Path
ROOT=Path(__file__).resolve().parent

def verify(stdout,stderr,status,expected):
    assert status==expected['exit'],(status,expected['exit'],stderr)
    assert stderr==b'',stderr
    if status:
        assert stdout==b'INVALID molecular input\n',stdout
        return 0
    lines=stdout.decode().splitlines()
    assert lines[0]=='molecular_pair_distribution bin count g coordination',stdout
    assert len(lines)==1+len(expected['rows']),stdout
    error=Decimal(0)
    for index,(line,row) in enumerate(zip(lines[1:],expected['rows'])):
        ordinal,count,g,coordination=line.split()
        assert int(ordinal)==index and int(count)==row['count'],(line,row)
        for actual,key in [(g,'g'),(coordination,'coordination')]:
            value=Decimal(actual); assert value.is_finite()
            difference=abs(value-Decimal(row[key]));error=max(error,difference)
            assert difference<=Decimal('1e-8'),(index,key,value,row[key],difference)
    return str(error)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--executable',type=Path);parser.add_argument('--captures',type=Path)
    options=parser.parse_args();assert bool(options.executable)!=bool(options.captures)
    for line in (ROOT/'data/FROZEN.sha256').read_text().splitlines():
        digest,name=line.split('  ',1);assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
    cases=json.loads((ROOT/'data/oracles.json').read_text());rows=[]
    output=options.captures or ROOT/'build/validation/native';output.mkdir(parents=True,exist_ok=True)
    for name,expected in cases.items():
        started=time.monotonic()
        if options.executable:
            completed=subprocess.run([str(options.executable.resolve())],input=(ROOT/f'data/{name}.stdin').read_bytes(),capture_output=True,timeout=60)
            stdout,stderr,status=completed.stdout,completed.stderr,completed.returncode
            (output/f'{name}.stdout').write_bytes(stdout);(output/f'{name}.stderr').write_bytes(stderr);(output/f'{name}.exit').write_text(str(status))
        else:
            stdout=(output/f'{name}.stdout').read_bytes();stderr=(output/f'{name}.stderr').read_bytes();status=int((output/f'{name}.exit').read_text())
        error=verify(stdout,stderr,status,expected)
        rows.append(dict(case=name,exit=status,max_error=error,seconds=time.monotonic()-started))
        print(name,'PASS',error,flush=True)
    # Metamorphic equality is asserted on the actual engine bytes.
    for name in ['translated','permuted']:
        assert (output/f'{name}.stdout').read_bytes()==(output/'wrapping.stdout').read_bytes()
    (output/'verified.json').write_text(json.dumps(rows,indent=2)+'\n')
if __name__=='__main__':main()
