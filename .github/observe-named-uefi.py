import pathlib, subprocess, tempfile, sys
root = pathlib.Path(tempfile.mkdtemp(prefix='omega-fixed-image-'))
cli = str(pathlib.Path(sys.argv[1]).resolve())
def run(name, args):
    p = subprocess.run([cli,*map(str,args)],capture_output=True,text=True)
    print(f'[{name}] exit={p.returncode}\n{p.stdout}{p.stderr}',flush=True)
    return p
def source(name, target, load, trait, provider, body, implementation=''):
    d=root/name
    d.mkdir()
    (d/'build.omg').write_text(load+f'\nmachine build(builder: @mut Build) {{\n    Build::package(builder, "{name}");\n    Build::select_provider::<{trait}, {provider}>(builder, {target});\n}}\n')
    (d/'main.omg').write_text(body+'\n'+implementation)
    artifact=d/'image.psi'
    p=run(name,['--emit-psi',artifact,'--target',target,d/'build.omg'])
    if p.returncode==0:
        print(f'[{name}] published {artifact.stat().st_size} bytes',flush=True)
        run(name+'-inspect',['psi','inspect','--artifact',artifact])
    return p
imports='use omega\\language\\core\\ProgramImage;\n'
image=imports+'public machine image_tail() -> addr reaches ProgramImage crashes none { ProgramImage::retained_suffix_start() }'
load=imports+'use omega\\language\\core\\ProgramImageNativeProvider;'
good=source('fixed_image','uefi_x86_64',load,'ProgramImage','ProgramImageNativeProvider',image)
host=source('wrong_host','linux_x86_64',load,'ProgramImage','ProgramImageNativeProvider',image)
fake=source('unsealed','uefi_x86_64',imports,'ProgramImage','Impostor',image,'public Impostor satisfies ProgramImage { boundary machine retained_suffix_start() -> addr satisfies ProgramImage::retained_suffix_start reaches none crashes none; }')
replacement=source('hosted_policy','linux_x86_64',imports,'ProgramImage','HostedImage',image,'public HostedImage satisfies ProgramImage { machine retained_suffix_start() -> addr reaches none crashes none { 9 } }')
storageimports='use omega\\language\\core\\ProgramStorageEntry;\n'
storage=storageimports+'use omega\\language\\core\\Extent;\nuse omega\\language\\core\\Extent\\Granted;\npublic machine enter_image(image: Extent in Granted, storage: Extent in Granted) reaches ProgramStorageEntry crashes Trap crashes Abort { ProgramStorageEntry::enter(image, storage); }'
handoff=source('fixed_storage','uefi_x86_64',storageimports+'use omega\\language\\core\\ProgramStorageEntryNativeProvider;','ProgramStorageEntry','ProgramStorageEntryNativeProvider',storage)
print(f'VERDICTS uefi={good.returncode} hosted={host.returncode} copied={fake.returncode} hosted_body={replacement.returncode} storage={handoff.returncode}',flush=True)
if not (good.returncode==0 and host.returncode==0 and fake.returncode!=0 and "supply mode other than a checked body" in fake.stderr and replacement.returncode==0 and handoff.returncode!=0 and "complete retained structural boundary contract" in handoff.stderr):
    sys.exit(1)
