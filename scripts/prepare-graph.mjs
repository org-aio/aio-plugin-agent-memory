import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { homedir } from 'node:os';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const lock = JSON.parse(readFileSync(join(root, 'graph/source.lock.json'), 'utf8'));
if (!/^[a-f0-9]{40}$/.test(lock.revision)) throw new Error('Graph revision must be a complete Git SHA');
let repo = process.env.AIO_GRAPH_SOURCE;
if (!repo) {
  repo = join(homedir(), '.cache/aio/sources', createHash('sha256').update(lock.git).digest('hex'));
  mkdirSync(repo, { recursive: true });
  execFileSync('git', ['init', '-q', repo]);
  try { execFileSync('git', ['-C', repo, 'cat-file', '-e', `${lock.revision}^{commit}`], { stdio: 'ignore' }); }
  catch {
    try { execFileSync('git', ['-C', repo, 'fetch', '--filter=blob:none', '--depth=1', lock.git, lock.revision], { stdio: 'inherit' }); }
    catch {
      throw new Error('Cannot fetch the locked graph dependency. az-compose is private: configure read-only Git access or set AIO_GRAPH_SOURCE to an authorized local checkout. See README.md.');
    }
  }
}
const working = process.argv.includes('--working-tree');
if (working && !process.env.AIO_GRAPH_SOURCE) throw new Error('--working-tree requires AIO_GRAPH_SOURCE');
for (const file of lock.files) {
  const source = `${lock.root}/${file}`;
  const bytes = working ? readFileSync(join(repo, source)) : execFileSync('git', ['-C', repo, 'show', `${lock.revision}:${source}`]);
  const output = join(root, 'graph/src/site/addzero/component/knowledge_graph', file);
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, bytes);
}
console.log(`Graph dependency: ${working ? 'LOCAL WORKING TREE (development only)' : lock.revision}`);
