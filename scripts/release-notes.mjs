import { readFileSync, writeFileSync } from 'node:fs';
const version = JSON.parse(readFileSync(new URL('../package.json', import.meta.url))).version;
const changelog = readFileSync(new URL('../CHANGELOG.md', import.meta.url), 'utf8');
const heading = `## ${version} — `;
const start = changelog.indexOf(heading);
if (start < 0) throw new Error(`Missing reviewed changelog entry for ${version}`);
const end = changelog.indexOf('\n## ', start + heading.length);
const notes = changelog.slice(start, end < 0 ? undefined : end).trim();
writeFileSync(new URL('../.release-notes.md', import.meta.url), `${notes}\n`);
// A reviewed "(pre-release)" marker in the changelog heading publishes a GitHub pre-release.
const headingLine = notes.split('\n', 1)[0] ?? '';
const prerelease = /\(pre-release\)\s*$/u.test(headingLine);
if (process.env.GITHUB_OUTPUT)
  writeFileSync(process.env.GITHUB_OUTPUT, `prerelease=${prerelease}\n`, { flag: 'a' });
console.log(`Release notes for ${version}${prerelease ? ' (pre-release)' : ''}`);
