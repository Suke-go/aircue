const fs = require('node:fs');
const version = JSON.parse(fs.readFileSync('package.json')).version;
const config = JSON.parse(fs.readFileSync('tauri.conf.json'));
const cargo = fs.readFileSync('Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
if (config.version !== version || cargo !== version || config.app.windows[0].title !== `AirCue ${version}`) {
  throw Error('package.json, Cargo.toml, tauri.conf.json and window title must have the same version');
}
if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${version}`) {
  throw Error(`Release tag must be v${version}`);
}
console.log(`AirCue ${version}: version check passed`);
