const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const app = fs.readFileSync('web/app.js','utf8');
const ctx = vm.createContext({});
vm.runInContext(app.slice(app.indexOf('function length('),app.indexOf('function persistDraft(')),ctx);
test('SHITARA editor waveform matches negative and positive 100 ms phases',()=>{
  const p={kind:'bipolar',widthMs:100,startMs:0,amplitude:100,polarity:1};
  const w={name:'SHITARA',parts:[p],levelDb:-18};
  assert.equal(ctx.duration(w),200);
  assert.equal(ctx.waveLimit(w),200);
  for(const t of [0,50,99.99])assert.equal(ctx.value(p,t),-1);
  for(const t of [100,150,199.99])assert.equal(ctx.value(p,t),1);
  assert.equal(ctx.value(p,200),0);
  assert.equal(ctx.check(w),w);
  p.startMs=.1;assert.throws(()=>ctx.check(w),/200 ms/);
});
test('existing short waveform limit stays at 80 ms',()=>{
  const p={kind:'push',pushMs:12,returnMs:48,startMs:20};
  const w={name:'existing',parts:[p]};
  assert.equal(ctx.waveLimit(w),80);
  ctx.check(w);
  p.startMs=20.1;assert.throws(()=>ctx.check(w),/80 ms/);
});
