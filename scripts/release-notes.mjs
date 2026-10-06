import { readFileSync, writeFileSync } from 'node:fs';
const version = JSON.parse(readFileSync(new URL('../package.json', import.meta.url))).version;
const changelog = readFileSync(new URL('../CHANGELOG.md', import.meta.url), 'utf8');
const heading = `## ${version} — `;
const start = changelog.indexOf(heading);
if (start < 0) throw new Error(`Missing reviewed changelog entry for ${version}`);
const end = changelog.indexOf('\n## ', start + heading.length);
const notes = changelog.slice(start, end < 0 ? undefined : end).trim();
writeFileSync(new URL('../.release-notes.md', import.meta.url), `${notes}\n`);
