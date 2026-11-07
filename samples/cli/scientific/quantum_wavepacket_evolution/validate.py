"""Validate numerical stdout from fresh native or actual Terminal executions."""
import argparse,hashlib,json,math,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--executable',type=Path)
    parser.add_argument('--terminal-sponsor',type=Path)
    parser.add_argument('--captures',type=Path)
    parser.add_argument('--timeout',type=int,default=1800)
    args=parser.parse_args()
    assert bool(args.executable)!=bool(args.terminal_sponsor)
    for line in (ROOT/'data/FROZEN.sha256').read_text().splitlines():
        digest,path=line.split('  ',1)
        assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==digest,path
    roster=json.loads((ROOT/'data/oracles.json').read_text())['cases']
    output=ROOT/'build/validation';output.mkdir(parents=True,exist_ok=True)
    route='terminal' if args.terminal_sponsor else 'native'
    captures=args.captures.resolve() if args.captures else Path(tempfile.mkdtemp(prefix=route+'-',dir=output))
    binary=args.executable or args.terminal_sponsor
    identity=hashlib.sha256(binary.read_bytes()).hexdigest()
    oracle_hash=hashlib.sha256((ROOT/'data/FROZEN.sha256').read_bytes()).hexdigest()
    if args.captures:
        receipt=json.loads((captures/'capture-receipt.json').read_text())
        assert receipt['binary_sha256']==identity and receipt['oracles_sha256']==oracle_hash and receipt['completed']
    names=captures/'names.txt';names.write_text(''.join(c['name']+'\n' for c in roster))
    if args.terminal_sponsor and not args.captures:
        completed=subprocess.run([str(args.terminal_sponsor.resolve()),str(ROOT),str(names),str(captures)],capture_output=True,timeout=args.timeout)
        (captures/'sponsor.stdout').write_bytes(completed.stdout);(captures/'sponsor.stderr').write_bytes(completed.stderr)
        assert completed.returncode==0,completed.stderr.decode(errors='replace')[-4000:]
    records=[];phase={};exponential_errors={}
    for case in roster:
        name=case['name'];started=time.monotonic()
        if args.executable:
            completed=subprocess.run([str(args.executable.resolve())],input=(ROOT/'data'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=args.timeout)
            stdout,stderr,exit_code=completed.stdout,completed.stderr,completed.returncode
            (captures/f'{name}.stdout').write_bytes(stdout);(captures/f'{name}.stderr').write_bytes(stderr);(captures/f'{name}.exit').write_text(str(exit_code))
        else:
            stdout=(captures/f'{name}.stdout').read_bytes();stderr=(captures/f'{name}.stderr').read_bytes();exit_code=int((captures/f'{name}.exit').read_text())
        assert exit_code==case['exit'] and not stderr,(name,exit_code,stdout[:500],stderr)
        record={'name':name,'exit':exit_code,'seconds':time.monotonic()-started}
        if exit_code:
            assert stdout==(b'REFUSED quantum conditioning\n' if exit_code==3 else b'INVALID quantum input\n'),(name,stdout)
        else:
            lines=stdout.decode().splitlines();assert lines[0]=='quantum_wavepacket_evolution hbar=m=1'
            n=case['cells']
            tokens=list(map(float,(ROOT/'data'/f'{name}.stdin').read_text().split()))
            # The frozen dense matvec energy suffers cancellation at h=1e-8
            # for the constant zero mode. Its independent analytic eigenvalue
            # gives the exact expectation without altering any frozen file.
            analytic_energy=None
            if case.get('mode') is not None:
                analytic_energy=(1-math.cos(2*math.pi*case['mode']/n))/case['spacing']**2+tokens[4]
            initial_energy=analytic_energy if analytic_energy is not None else case['checkpoints'][0]['energy']
            assert len(lines)==1+(n+1)*(case['steps']+1),(name,len(lines))
            maximum=0.;drift=0.;energy_drift=0.
            for index,reference in enumerate(case['checkpoints']):
                values=lines[1+index*(n+1):1+(index+1)*(n+1)]
                iteration_text,norm_text,energy_text=values[0].split();iteration=int(iteration_text);norm=float(norm_text);energy=float(energy_text)
                assert iteration==index and math.isfinite(norm) and math.isfinite(energy)
                drift=max(drift,abs(norm-1.));energy_drift=max(energy_drift,abs(energy-initial_energy))
                amplitude=[];probability=[]
                for line in values[1:]:
                    real,imaginary,weight=map(float,line.split())
                    assert all(map(math.isfinite,(real,imaginary,weight))) and weight>=0
                    amplitude.append(complex(real,imaginary));probability.append(weight)
                error=max(abs(z-complex(*r)) for z,r in zip(amplitude,reference['amplitude']))
                maximum=max(maximum,error)
                exact=[complex(*r) for r in reference['exponential']]
                truncation=max(abs(complex(*a)-b) for a,b in zip(reference['amplitude'],exact))
                observed_exact=max(abs(a-b) for a,b in zip(amplitude,exact))
                assert abs(observed_exact-truncation)<=1e-8,(name,index,'matrix exponential',observed_exact,truncation)
                if index==case['steps']:exponential_errors[name]=observed_exact
                assert error<=1e-8,(name,index,'solver',error)
                assert abs(norm-reference['norm'])<=1e-8
                assert abs(energy-(analytic_energy if analytic_energy is not None else reference['energy']))<=1e-8
                assert max(abs(a-b) for a,b in zip(probability,reference['probability']))<=1e-8
                assert abs(sum(probability)-norm)<=1e-8
                if name.startswith('eigen_') and index==case['steps']:
                    exact=[complex(*r) for r in reference['analytic']]
                    overlap=sum(a.conjugate()*b for a,b in zip(exact,amplitude))*case['spacing']
                    phase[name]=abs(math.atan2(overlap.imag,overlap.real))
            assert drift<=1e-8 and energy_drift<=1e-8,(name,drift,energy_drift)
            record.update(solver_error=maximum,norm_drift=drift,energy_drift=energy_drift)
        records.append(record);print(route,name,'PASS',flush=True)
    assert 3.9<phase['eigen_coarse']/phase['eigen_half']<4.1,phase
    assert 3.9<phase['eigen_half']/phase['eigen_quarter']<4.1,phase
    for prefix in ('wavepacket','barrier'):
        coarse=exponential_errors[prefix+'_16'];half=exponential_errors[prefix+'_half'];quarter=exponential_errors[prefix+'_quarter']
        assert 3.8<coarse/half<4.2 and 3.8<half/quarter<4.2,(prefix,coarse,half,quarter)
    binary=args.executable or args.terminal_sponsor
    report={'engine':route,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'records':records,'phase_errors':phase,'matrix_exponential_errors':exponential_errors,'captures':str(captures)}
    if not args.captures:
        receipt={'binary_sha256':identity,'oracles_sha256':oracle_hash,'completed':True,'outputs':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in captures.iterdir() if path.suffix in ('.stdout','.stderr','.exit')}}
        (captures/'capture-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    else:
        for filename,digest in receipt['outputs'].items():assert hashlib.sha256((captures/filename).read_bytes()).hexdigest()==digest
    (output/f'{route}-results.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'{route}: {len(records)} cases; solver <=1e-8; norm drift <=1e-8; phase-halving ratios verified')
if __name__=='__main__':main()
