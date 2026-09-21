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
test('mirror and reverse preserve the trajectory under the corresponding transform',()=>{
  for(const id of ['above','approach','orbit']) {
    const d=structuredClone(base);Spatial.applyPreset(d,id);const original=structuredClone(d);
    Spatial.transform(d,'mirror');
    for(const t of [0,.2,.7,1]) assert.equal(Spatial.pose(d,t).azimuth,Spatial.wrap(-Spatial.pose(original,t).azimuth));
    Spatial.transform(d,'mirror');Spatial.transform(d,'reverse');
    for(const t of [0,.2,.7,1]) {
      const a=Spatial.pose(d,t),b=Spatial.pose(original,1-t);
      for(const k of ['azimuth','elevation','distanceM'])assert.ok(Math.abs(a[k]-b[k])<1e-8);
    }
  }
});
test('spacing and loop closure leave the source and gain unchanged',()=>{
  const d=structuredClone(base);Spatial.applyPreset(d,'above');d.keyframes[1].at=.2;
  Spatial.transform(d,'space');assert.equal(d.keyframes[1].at,.5);
  Spatial.transform(d,'close');assert.deepEqual({...d.keyframes[2],at:0},d.keyframes[0]);
  assert.equal(d.source,base.source);assert.equal(d.levelDb,base.levelDb);
});
test('history supports independent snapshots, redo and branching',()=>{
  const h=Spatial.history(2),d=structuredClone(base);h.reset(d);
  d.azimuth=45;h.record(d);d.azimuth=90;h.record(d);
  const undo=h.undo();assert.equal(undo.azimuth,45);undo.azimuth=100;
  assert.equal(h.redo().azimuth,90);assert.equal(h.undo().azimuth,45);
  h.record({...d,azimuth:-90});assert.equal(h.canRedo,false);assert.equal(h.undo().azimuth,45);
});
