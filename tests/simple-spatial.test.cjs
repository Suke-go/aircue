const {test}=require('node:test');
const assert=require('node:assert/strict');
global.Spatial=require('../web/spatial.js');
const Simple=require('../web/simple-spatial.js');
const design=()=>structuredClone(require('./unity-project.json').audioClips[0].design);
test('all simple presets map to Design and retain source, level and duration',()=>{
  for(const id of Object.keys(Simple.names)){
    const d=design(),before=structuredClone(d);Simple.apply(d,id);
    assert.equal(Simple.identify(d),id);
    for(const key of ['source','assetId','levelDb','durationMs','offsetMs','frequency'])assert.deepEqual(d[key],before[key]);
    for(let i=0;i<=100;i++){const p=Spatial.pose(d,i/100);assert.ok(p.distanceM>=.3&&p.distanceM<=5);}
  }
});
test('left/right pass direction and rear arc are unambiguous',()=>{
  const d=design();Simple.apply(d,'reversePass');assert.equal(Spatial.pose(d,0).azimuth,90);assert.equal(Spatial.pose(d,1).azimuth,-90);
  Simple.apply(d,'rearPass');assert.equal(Spatial.pose(d,.5).azimuth,-180);
});
test('custom path recognition and repeated mode reads do not mutate any data',()=>{
  const d=design();d.motion='path';d.keyframes=[{at:0,azimuth:23,elevation:11,distanceM:.7},{at:.3,azimuth:150,elevation:50,distanceM:4},{at:1,azimuth:-100,elevation:-30,distanceM:2}];
  const before=structuredClone(d);for(let i=0;i<20;i++)assert.equal(Simple.identify(d),'');assert.deepEqual(d,before);
  Simple.move(d,-40,1);assert.deepEqual(d.keyframes.map(k=>[k.at,k.elevation]),before.keyframes.map(k=>[k.at,k.elevation]));
  for(let i=1;i<3;i++)assert.ok(Math.abs(d.keyframes[i].distanceM/d.keyframes[0].distanceM-before.keyframes[i].distanceM/before.keyframes[0].distanceM)<1e-10);
});
test('spatial off keeps the same trajectory and distance edits respect all path limits',()=>{
  const d=design();Simple.apply(d,'approach');d.spatial=false;const original=structuredClone(d);Simple.identify(d);assert.deepEqual(d,original);
  Simple.move(d,90,5);assert.equal(d.spatial,false);assert.equal(Spatial.pose(d,0).azimuth,90);assert.equal(Spatial.pose(d,0).distanceM,5);assert.equal(Spatial.pose(d,1).distanceM,.5);
});
test('rotating a legacy long sweep across the seam retains its long arc',()=>{
  const d=design();Object.assign(d,{motion:'sweep',azimuth:-160,endAzimuth:160});const before=structuredClone(d);
  Simple.move(d,170,1);assert.equal(d.motion,'path');
  for(let i=0;i<=100;i++)assert.ok(Math.abs(Spatial.wrap(Spatial.pose(d,i/100).azimuth-Spatial.pose(before,i/100).azimuth+30))<1e-8);
});
test('short custom deviations are never incorrectly recognized as a preset',()=>{
  const d=design();Simple.apply(d,'pass');d.keyframes.splice(1,0,{at:.01,azimuth:20,elevation:0,distanceM:1});assert.equal(Simple.identify(d),'');
  Simple.apply(d,'approach');Simple.distance(d,1);assert.equal(Math.max(...d.keyframes.map(p=>p.distanceM)),1);assert.ok(Math.min(...d.keyframes.map(p=>p.distanceM))>=.3);
});
test('A/B preview uses copies and never changes saved spatial state or custom path',()=>{
  const vm=require('node:vm'),fs=require('node:fs');const ctx=vm.createContext({Spatial,structuredClone});vm.runInContext(fs.readFileSync(require.resolve('../web/simple-spatial.js'),'utf8'),ctx);
  const d=design();Simple.apply(d,'above');d.spatial=false;ctx.saved=d;const before=structuredClone(d);
  assert.equal(vm.runInContext("audioAB='spatial';audioPreviewDesign(saved).spatial",ctx),true);
  assert.equal(vm.runInContext("audioAB='original';audioPreviewDesign(saved).spatial",ctx),false);
  assert.deepEqual(d,before);
});
