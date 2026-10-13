"""Copy the verified CLI and its sponsor closure into a private immutable root."""
import argparse, hashlib, json, shutil
from pathlib import Path

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',type=Path,required=True)
    parser.add_argument('--receipt',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    options=parser.parse_args()
    binary=options.omega.resolve();release=binary.parent
    receipt=json.loads(options.receipt.read_text())
    assert digest(binary)==receipt['compiler_binary_sha256']
    destination=options.output.resolve()/'release'
    assert not destination.exists(), 'choose a fresh private producer directory'
    destination.mkdir(parents=True)
    sources={binary}
    metadata=Path(receipt['cli_unit_metadata'])
    sources.update([metadata,metadata.with_suffix(''),metadata.parent.parent/'out'/binary.name])
    for library in receipt['libraries']:
        path=Path(library['path']);assert digest(path)==library['sha256']
        sources.add(path)
        full=path.with_suffix('.rmeta')
        if full.exists():sources.add(full)
        fingerprint=path.parent.parent/'fingerprint'/('lib-'+library['name'])
        sources.update([fingerprint,fingerprint.with_suffix('.json')])
    identities={}
    for source in sorted(sources):
        target=destination/source.relative_to(release)
        target.parent.mkdir(parents=True,exist_ok=True)
        before=digest(source);shutil.copy2(source,target)
        assert digest(source)==before==digest(target), ('artifact changed during freeze',source)
        identities[str(target)]={'origin':str(source),'sha256':before}
    result={'binary':str(destination/binary.name),'original_receipt':str(options.receipt.resolve()),
            'artifacts':identities,'compiler_binary_sha256':receipt['compiler_binary_sha256']}
    (options.output/'freeze.json').write_text(json.dumps(result,indent=2)+'\n')
    print(result['binary'])
if __name__=='__main__': main()
