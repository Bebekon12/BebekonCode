import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';

const cargo = join(homedir(), '.cargo', 'bin');
const cli = join(import.meta.dirname, '..', 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const env = { ...process.env };
const pathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path');
const originalPath = pathKey ? env[pathKey] : '';
if (pathKey) delete env[pathKey];
env.PATH = existsSync(cargo) ? `${cargo}${delimiter}${originalPath ?? ''}` : originalPath;
const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], { stdio: 'inherit', env });
child.on('error', (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
