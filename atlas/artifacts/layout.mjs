// Layout runs during publication. The deployed page only needs the SVG and D3.
import fs from 'node:fs';
import ELK from 'elkjs/lib/elk.bundled.js';

const model = JSON.parse(fs.readFileSync(0, 'utf8'));
const nodes = new Map(model.nodes.map(n => [n.id, n]));
const options = {
  'elk.algorithm': 'layered',
  'elk.edgeRouting': 'ORTHOGONAL',
  'elk.spacing.nodeNode': '55',
  'elk.layered.spacing.nodeNodeBetweenLayers': '70',
  'elk.layered.wrapping.strategy': 'MULTI_EDGE',
  'elk.aspectRatio': '1.8',
  'elk.padding': '[top=70,left=35,bottom=35,right=35]',
};
const source = {id:'source-group', title:'Psi · source compilation', layoutOptions:options, children:[], edges:[]};
// Keep auxiliary native inputs and their consumers in the same wrapped section.
// Cutting several native connections together can route an input through a form.
const native = {id:'native-group', title:'Omega · native compilation',
  layoutOptions:{...options,'elk.layered.wrapping.strategy':'SINGLE_EDGE'}, children:[], edges:[]};
const runtime = {id:'runtime-group', title:'Evaluation and Psi execution', layoutOptions:options, children:[], edges:[]};
const root = {
  id:'root',
  layoutOptions:{...options, 'elk.direction':'DOWN',
    'elk.layered.wrapping.strategy':'OFF', 'elk.hierarchyHandling':'INCLUDE_CHILDREN'},
  children:[source,native,runtime], edges:[],
};
const parents = new Map();
function label(id, text, max=30) {
  const words=text.split(' '), lines=[];
  let line='';
  for(const word of words) {
    if(line && (line+' '+word).length>max) {lines.push(line);line='';}
    line+=(line?' ':'')+word;
  }
  if(line) lines.push(line);
  return {id,text,lines,width:Math.max(...lines.map(s=>s.length))*9+8,height:lines.length*26};
}
for(const node of model.nodes) {
  const parent = node.group==='source' ? source :
    ['native','encoding','emission','image'].includes(node.group) ? native :
    ['runtime','evaluation'].includes(node.group) ? runtime : root;
  parents.set(node.id,parent);
  const transform=node.role==='transform';
  parent.children.push({id:node.id, width:transform?12:node.width, height:transform?12:node.height,
    ...(transform?{labels:[{...label(node.id+'-label',node.title,24),
      layoutOptions:{'elk.nodeLabels.placement':'OUTSIDE V_TOP H_CENTER'}}]}:{})});
}
for(const edge of model.edges) {
  const parent=parents.get(edge.start)===parents.get(edge.end)?parents.get(edge.start):root;
  const showLabel=edge.label && nodes.get(edge.start).role!=='transform';
  parent.edges.push({id:edge.id, sources:[edge.start], targets:[edge.end],
    ...(showLabel?{labels:[label(edge.id+'-label',edge.label)]}:{}),
    layoutOptions:{'elk.layered.priority.direction':edge.kind==='result'?'0':'10'}});
}
const graph=await new ELK().layout(root);
const result={width:graph.width,height:graph.height,nodes:[],edges:[],groups:[]};
function flatten(group,offsetX=0,offsetY=0) {
  const x=offsetX+(group.x||0),y=offsetY+(group.y||0);
  if(group.id!=='root')result.groups.push({id:group.id,title:group.title,x,y,width:group.width,height:group.height});
  for(const node of group.children||[]) {
    if(node.children)flatten(node,x,y);
    else result.nodes.push({...node,x:x+node.x,y:y+node.y,
      labels:(node.labels||[]).map(l=>({...l,x:x+node.x+l.x,y:y+node.y+l.y}))});
  }
  for(const edge of group.edges||[])result.edges.push({...edge,
    sections:(edge.sections||[]).map(s=>({...s,
      points:[s.startPoint,...(s.bendPoints||[]),s.endPoint].map(p=>({x:x+p.x,y:y+p.y}))})),
    labels:(edge.labels||[]).map(l=>({...l,x:x+l.x,y:y+l.y}))});
}
flatten(graph);
process.stdout.write(JSON.stringify(result));
