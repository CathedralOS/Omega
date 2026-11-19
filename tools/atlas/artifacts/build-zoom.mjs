import fs from 'node:fs';
import {spawnSync} from 'node:child_process';

const directory=new URL('.',import.meta.url);
const result=spawnSync(process.execPath,['build','zoom-entry.js','--minify','--outfile','../design/diagram-zoom.js'],{cwd:directory,stdio:'inherit'});
if(result.status!==0)process.exit(result.status||1);
const packages=fs.readdirSync(new URL('node_modules/',directory))
  .filter(name=>name.startsWith('d3-')).sort();
const licenses=packages.map(name=>{
  const folder=new URL(`node_modules/${name}/`,directory);
  const pkg=JSON.parse(fs.readFileSync(new URL('package.json',folder),'utf8'));
  return `${name} ${pkg.version}\n${pkg.homepage}\n\n${fs.readFileSync(new URL('LICENSE',folder),'utf8')}`;
}).join('\n\n---\n\n');
fs.writeFileSync(new URL('../design/diagram-zoom-LICENSE.txt',directory),licenses);
const bundle=new URL('../design/diagram-zoom.js',directory);
fs.writeFileSync(bundle,'/*! D3 modules: see diagram-zoom-LICENSE.txt for copyright and licenses. */\n'+fs.readFileSync(bundle,'utf8'));
