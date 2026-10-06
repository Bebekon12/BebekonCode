import { readFileSync } from 'node:fs';
const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url)));
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
const workspaceVersion = cargo.match(/\[workspace.package\]\s+version = "([^"]+)"/u)?.[1];
if (pkg.version !== workspaceVersion)
  throw new Error('package.json and Cargo workspace versions must match');
if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${pkg.version}`) {
  throw new Error('Release tag must match the application version');
}
console.log(`Version ${pkg.version} is consistent`);
