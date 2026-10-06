import fs from 'node:fs/promises';
import path from 'node:path';

const pkg = JSON.parse(await fs.readFile('package.json', 'utf8'));
const product = JSON.parse(await fs.readFile('product.json', 'utf8'));
const filename = `BebekonCode_${pkg.version}_x64-setup.exe`;
const installer = path.resolve('target/release/bundle/nsis', filename);
const signature = (await fs.readFile(`${installer}.sig`, 'utf8')).trim();
if (
  !signature ||
  !Buffer.from(signature, 'base64').toString('utf8').startsWith('untrusted comment:')
) {
  throw new Error('Signed installer required');
}
await fs.access(installer);
const target = {
  url: `https://github.com/${product.repository}/releases/download/v${pkg.version}/${filename}`,
  signature,
};
const manifest = {
  version: pkg.version,
  notes: await fs.readFile('.release-notes.md', 'utf8'),
  pub_date: new Date().toISOString(),
  platforms: { 'windows-x86_64': target, 'windows-x86_64-nsis': target },
};
await fs.writeFile('target/release/latest.json', JSON.stringify(manifest, null, 2) + '\n');
console.log(`Prepared signed update manifest for ${pkg.version}`);
