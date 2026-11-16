import { copyFileSync, mkdirSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join, basename } from 'node:path';
import assert from 'node:assert/strict';
const latest = resolve('artifacts/candidates/latest-candidate.json');
const report = JSON.parse(readFileSync(latest, 'utf8'));
const hash = file => createHash('sha256').update(readFileSync(file)).digest('hex');
assert.equal(hash(join(report.directory, 'source-manifest.json')), report.sourceManifestSha256);
for (const input of JSON.parse(readFileSync(join(report.directory, 'source-manifest.json'), 'utf8'))) {
  assert.equal(hash(join(report.source, input.path)), input.sha256, `Frozen source changed: ${input.path}`);
}
const generated = join(report.targetDirectory, 'release/nsis/x64/installer.nsi');
const script = readFileSync(generated, 'utf8');
assert.ok(script.includes(`!define PRODUCTNAME "${report.productName}"`));
assert.ok(script.includes(`!define BUNDLEID "${report.identifier}"`));
assert.ok(script.includes('!define INSTALLMODE "currentUser"'));
assert.ok(script.includes('"/NS"') && script.includes('"/R"'));
assert.ok(!readFileSync(join(report.source, 'candidate.conf.json'), 'utf8').includes('remote-debugging'));
const artifacts = join(report.directory, 'package'); mkdirSync(artifacts);
const sources = [
  join(report.targetDirectory, 'release/bundle/nsis', `${report.productName}_${report.version}_x64-setup.exe`),
  join(report.targetDirectory, 'release/tinixcut.exe'), generated,
  join(report.source, 'src-tauri/resources/bin/ffmpeg.exe'),
  join(report.source, 'src-tauri/resources/bin/ffprobe.exe'),
  join(report.source, 'src-tauri/resources/ffmpeg_provenance.json'),
];
report.artifacts = [];
for (const source of sources) {
  const destination = join(artifacts, basename(source)); copyFileSync(source, destination);
  report.artifacts.push({ name: basename(source), path: destination, bytes: statSync(destination).size, sha256: hash(destination) });
}
report.status = 'built'; report.builtAt = new Date().toISOString(); report.frozenInputsVerified = true;
report.installer = report.artifacts[0]; report.binary = report.artifacts[1];
// Tauri temporarily patches this marker while producing NSIS, then restores the
// standalone release executable. Predict the payload hash without mutating it.
const standalone = readFileSync(report.binary.path);
const marker = Buffer.from('__TAURI_BUNDLE_TYPE_VAR_UNK');
const offset = standalone.indexOf(marker);
assert.ok(offset >= 0 && standalone.indexOf(marker, offset + 1) === -1, 'Expected one Tauri bundle marker');
const nsisPayload = Buffer.from(standalone);
Buffer.from('__TAURI_BUNDLE_TYPE_VAR_NSS').copy(nsisPayload, offset);
report.expectedInstalledBinary = { sha256: createHash('sha256').update(nsisPayload).digest('hex'), bytes: nsisPayload.length, bundleMarker: { offset, standalone: 'UNK', nsis: 'NSS' } };
writeFileSync(join(report.directory, 'candidate.json'), JSON.stringify(report, null, 2));
writeFileSync(latest, JSON.stringify(report, null, 2));
console.log(JSON.stringify({ status: report.status, installer: report.installer, binary: report.binary, frozenInputsVerified: true }, null, 2));
