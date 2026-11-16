import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync, statSync, symlinkSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join, relative } from 'node:path';
import { spawnSync } from 'node:child_process';

const workspace = process.cwd();
const id = new Date().toISOString().replace(/[-:.]/g, '').replace('T', '-').replace('Z', '').toLowerCase();
const directory = resolve('artifacts/candidates', id);
const source = join(directory, 'source');
mkdirSync(source, { recursive: true });
const inputs = ['src', 'public', 'index.html', 'package.json', 'pnpm-lock.yaml', 'tsconfig.json', 'tsconfig.node.json', 'vite.config.ts',
  'src-tauri/src', 'src-tauri/fixtures', 'src-tauri/resources', 'src-tauri/icons', 'src-tauri/capabilities', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/build.rs', 'src-tauri/tauri.conf.json'];
for (const name of inputs) {
  if (!existsSync(name)) { if (name === 'public') continue; throw new Error(`Candidate input missing: ${name}`); }
  cpSync(name, join(source, name), { recursive: true, dereference: false, errorOnExist: true });
}
const productName = `TinixCut Candidate ${id}`;
const identifier = `com.tinixcut.candidate${id.replaceAll('-', '')}`;
const config = {
  productName, identifier,
  app: { windows: [{ label: 'main', title: productName, width: 1280, height: 800, focus: false }] },
};
writeFileSync(join(source, 'candidate.conf.json'), JSON.stringify(config, null, 2));
const hash = file => createHash('sha256').update(readFileSync(file)).digest('hex');
const manifest = [];
const visit = directory => {
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symlink in candidate input: ${path}`);
    if (entry.isDirectory()) visit(path);
    else manifest.push({ path: relative(source, path).replaceAll('\\', '/'), bytes: statSync(path).size, sha256: hash(path) });
  }
};
visit(source);
const manifestPath = join(directory, 'source-manifest.json');
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
// Reuse the installed dependency tree read-only; pin both lockfiles and tool versions.
// No package install or dependency update occurs in the working checkout.
symlinkSync(resolve('node_modules'), join(source, 'node_modules'), 'junction');
const git = args => spawnSync('git', args, { encoding: 'utf8', windowsHide: true }).stdout.trim();
const report = {
  id, createdAt: new Date().toISOString(), directory, source, workspace,
  revision: git(['rev-parse', 'HEAD']), dirty: Boolean(git(['status', '--porcelain', '--untracked-files=no'])),
  sourceManifestSha256: hash(manifestPath), inputFiles: manifest.length,
  productName, identifier, version: JSON.parse(readFileSync('package.json', 'utf8')).version,
  targetDirectory: resolve('src-tauri/target'), status: 'prepared',
  build: 'Release NSIS from frozen inputs with isolated product/identifier and unfocused window; production CSP/resources/asset scope retained; no CDP or qualification permissions',
  dependencies: { node: process.version, cargo: spawnSync('cargo', ['--version'], { encoding: 'utf8', windowsHide: true }).stdout.trim(), pnpm: readFileSync('node_modules/.modules.yaml', 'utf8').match(/packageManager: (.+)/)?.[1] ?? 'see pnpm-lock.yaml', linkedNodeModules: resolve('node_modules') },
  limits: ['Candidate identity differs from production; install evidence does not qualify production identity or cross-version upgrade.', 'Dependency tree is reused by junction; source inputs and lockfiles are pinned, but byte-for-byte reproducibility is not claimed.', 'No GUI or native file chooser acceptance is implied by build/install.'],
};
writeFileSync(join(directory, 'candidate.json'), JSON.stringify(report, null, 2));
writeFileSync('artifacts/candidates/latest-candidate.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
