const {test} = require('node:test');
const assert = require('node:assert/strict');
const Spatial = require('../web/spatial.js');
const fixtures = require('./spatial-cases.json');
const base = {azimuth:0,elevation:0,distanceM:1,motion:'fixed',durationMs:3000,speed:45,endAzimuth:90,keyframes:[],spatial:false,source:'file',assetId:'preserve',levelDb:-18};
for (const c of fixtures.cases) test(c.name, () => {
  const p = Spatial.pose({...base,...c.patch},c.at);
  [p.azimuth,p.elevation,p.distanceM].forEach((v,i)=>assert.ok(Math.abs(v-c.expected[i])<1e-8));
});
test('inserting a point preserves the existing path and sorts times',()=>{
  const d=structuredClone(base);Spatial.applyPreset(d,'above');
  const expected=Spatial.pose(d,.3);const index=Spatial.insert(d,.3);
  assert.deepEqual(d.keyframes[index],expected);
  assert.deepEqual(Spatial.pose(d,.15),{at:.15,azimuth:-63,elevation:27,distanceM:1});
  const second=Spatial.insert(d,.3);assert.ok(second>0);
  assert.ok(d.keyframes.every((k,i)=>i===0||k.at>d.keyframes[i-1].at));
});
test('all presets preserve source, gain and duration',()=>{
  for(const id of Object.keys(Spatial.presets)) {
    const d=structuredClone(base);Spatial.applyPreset(d,id);
    for(const k of ['source','assetId','levelDb','durationMs']) assert.equal(d[k],base[k]);
    assert.equal(d.spatial,true);
    for(const t of [0,.25,.5,.75,1]) assert.ok(Object.values(Spatial.pose(d,t)).every(Number.isFinite));
  }
});
test('duration changes preserve normalized path positions',()=>{
  const d=structuredClone(base);Spatial.applyPreset(d,'pass');const before=Spatial.pose(d,.5);d.durationMs=6000;assert.deepEqual(Spatial.pose(d,.5),before);
});
test('converting legacy sweep to a path preserves its midpoint',()=>{
  const d={...structuredClone(base),motion:'sweep',azimuth:-90,endAzimuth:90};const middle=Spatial.pose(d,.5);Spatial.ensurePath(d);assert.deepEqual(Spatial.pose(d,.5),middle);
});
