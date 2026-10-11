"""Execute only the published Omega application, sequentially, finite input."""
import pathlib, subprocess, sys
binary=pathlib.Path(sys.argv[1]).resolve(); output=pathlib.Path(sys.argv[2]).resolve()
output.mkdir(parents=True,exist_ok=True)
inventory=pathlib.Path(__file__).with_name('input_cases.tsv')
cases={name:bytes.fromhex(data) for name,data,_ in (line.split('\t') for line in inventory.read_text().splitlines() if not line.startswith('#'))}
for name,data in cases.items():
    result=subprocess.run([str(binary)],input=data,capture_output=True,timeout=30)
    for suffix,value in [('stdin',data),('stdout',result.stdout),('stderr',result.stderr),('exit',f'{result.returncode}\n'.encode())]:
        (output/f'native-{name}.{suffix}').write_bytes(value)
    print(f'native case {name}: exit {result.returncode}')
