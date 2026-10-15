// Official offline session mutation only. Never calls query(), opens auth, or emits a prompt.
import { forkSession } from '@anthropic-ai/claude-agent-sdk';
const [id, directory] = process.argv.slice(2);
if (!id || !directory || !process.env.CLAUDE_CONFIG_DIR) process.exit(2);
try {
  const result = await forkSession(id, { dir: directory });
  process.stdout.write(JSON.stringify({ session_id: result.sessionId }));
} catch {
  process.exitCode = 1;
}
