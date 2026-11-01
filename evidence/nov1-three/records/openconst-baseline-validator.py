import argparse, subprocess, sys
p=argparse.ArgumentParser();p.add_argument('--cli',required=True);p.add_argument('--source',required=True);p.add_argument('--reject',action='store_true');a=p.parse_args()
cmd=[a.cli,'--check',a.source] if a.reject else [a.cli,'run','--both',a.source]
r=subprocess.run(cmd,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=65)
print(r.stdout,end='');print(r.stderr,end='',file=sys.stderr);print('CLI exit '+str(r.returncode),file=sys.stderr)
t=r.stdout+r.stderr
if a.reject:
    ok=r.returncode!=0 and 'expected a complete' in t and 'Reading' in t and 'conformance' in t
    print('fact-sensitive implementation tuple reject verified' if ok else 'authored implementation tuple mismatch refusal was not observed',file=sys.stderr)
    sys.exit(0 if ok else 3)
if r.returncode:sys.exit(r.returncode)
if 'native exit 0' not in t or 'interp exit: 0' not in t:sys.exit(4)
