"""Record exact Cargo/MBX compiler-artifact outputs for a verified producer build.
Feed the JSON stream from a successful `mbx build --locked -p omega --release
-j4 --message-format=json`; this helper requires its build-finished success row.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--manifest',type=Path,required=True)
    parser.add_argument('--producer-release',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--output',type=Path,required=True)
    options=parser.parse_args();selected={};finished=False;binary=None
    for line in options.manifest.read_text().splitlines():
        try:record=json.loads(line)
        except ValueError:continue
        if record.get('reason')=='build-finished':finished=record['success']
        if record.get('reason')!='compiler-artifact':continue
        name=record['target']['name']
        if name=='omega' and record.get('executable'):binary=Path(record['executable']).resolve()
        if name not in ('compiler', 'terminal_runtime','omega_build', 'package_manager','terminal_interpreter','target'):continue
        assert name not in selected,('duplicate producer artifact',name)
        selected[name]=[{'path':str(Path(path).resolve()),'sha256':digest(Path(path))}
                        for path in record['filenames'] if Path(path).suffix in ('.rlib','.rmeta')]
        assert selected[name],('missing linkable producer artifact',name)
    assert finished and len(selected)==4,'incomplete or unsuccessful producer build manifest'
    assert binary==(options.producer_release/'omega').resolve(),'producer binary differs from build manifest'
    version=subprocess.check_output([str(binary),'--version'],text=True).strip()
    assert options.commit in version,('producer commit mismatch',version)
    receipt={'commit':options.commit,'binary_version':version,'binary_sha256':digest(binary),
             'libraries':selected,'artifact_manifest_sha256':digest(options.manifest),'build_finished_success':finished}
    options.output.write_text(json.dumps(receipt,indent=2)+'\n')
    print('exact producer artifact receipt:',options.output)
if __name__=='__main__':main()
