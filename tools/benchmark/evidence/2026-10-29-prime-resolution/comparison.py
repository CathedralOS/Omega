import pathlib,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[2]
for label in ['baseline','candidate','candidate','baseline']:
    command=['python','build/prime-preparation-2/replay.py','prepare',label,'--both','--cold']
    result=subprocess.run(command,cwd=root)
    if result.returncode:sys.exit(result.returncode)
