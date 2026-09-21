'use strict';
const Spatial = (() => {
  const wrap = a => ((a + 180) % 360 + 360) % 360 - 180;
  const key = (at, azimuth, elevation = 0, distanceM = 1) => ({at, azimuth, elevation, distanceM});
  function pose(d, progress) {
    const at = Math.max(0, Math.min(1, progress));
    let p = key(at, d.azimuth, d.elevation, d.distanceM);
    if (d.motion === 'orbit') p.azimuth += d.speed * at * d.durationMs / 1000;
    if (d.motion === 'sweep') p.azimuth += (d.endAzimuth - d.azimuth) * at;
    if (d.motion === 'path' && d.keyframes?.length >= 2) {
      const keys = d.keyframes;
      let i = keys.findIndex(k => k.at >= at); if (i < 0) i = keys.length - 1; i = Math.max(1, i);
      const a = keys[i - 1], b = keys[i], t = (at - a.at) / (b.at - a.at);
      const raw = b.azimuth - a.azimuth; let delta = wrap(raw); if (delta === -180 && raw > 0) delta = 180;
      p = key(at, a.azimuth + delta * t, a.elevation + (b.elevation - a.elevation) * t, a.distanceM + (b.distanceM - a.distanceM) * t);
    }
    p.azimuth = wrap(p.azimuth); return p;
  }
  function ensurePath(d) {
    if (!d.keyframes?.length) d.keyframes = [pose(d, 0), pose(d, .5), pose(d, 1)];
    d.motion = 'path'; return d;
  }
  function insert(d, at) {
    if (d.keyframes.length >= 32) throw Error('経路は32点までです');
    const keys = d.keyframes;
    if (at <= .001 || at >= .999 || keys.some(k => Math.abs(k.at - at) < .001)) {
      let gap = 0; for (let i = 1; i < keys.length; i++) if (keys[i].at - keys[i - 1].at > gap) { gap = keys[i].at - keys[i - 1].at; at = (keys[i].at + keys[i - 1].at) / 2; }
    }
    const p = pose(d, at); const index = keys.findIndex(k => k.at > at); keys.splice(index, 0, p); return index;
  }
  const presets = {
    front: {name:'正面', azimuth:0, elevation:0, distanceM:1, motion:'fixed'},
    right: {name:'右', azimuth:90, elevation:0, distanceM:1, motion:'fixed'},
    rear: {name:'後方', azimuth:180, elevation:0, distanceM:1, motion:'fixed'},
    left: {name:'左', azimuth:-90, elevation:0, distanceM:1, motion:'fixed'},
    orbit: {name:'周回', azimuth:0, elevation:0, distanceM:1, motion:'orbit'},
    pass: {name:'左 → 正面 → 右', motion:'path', keyframes:[key(0,-90),key(.5,0),key(1,90)]},
    above: {name:'頭上を通過', motion:'path', keyframes:[key(0,-90),key(.5,0,90),key(1,90)]},
    approach: {name:'正面から接近', motion:'path', keyframes:[key(0,0,0,5),key(1,0,0,.5)]}
  };
  function applyPreset(d, id) {
    const p = presets[id]; if (!p) return;
    const {name, ...config} = p;
    Object.assign(d, structuredClone(config), {spatial:true});
    d.keyframes = p.keyframes ? structuredClone(p.keyframes) : [];
    // One complete orbit for ordinary clip durations; keep the existing angular-speed limit.
    if (id === 'orbit') d.speed = Math.min(360, 360000 / d.durationMs);
  }
  return {pose, wrap, ensurePath, insert, presets, applyPreset};
})();
if (typeof module !== 'undefined') module.exports = Spatial;
