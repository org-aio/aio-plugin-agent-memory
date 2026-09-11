import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const lock = JSON.parse(readFileSync('backend/contract/source.json', 'utf8'));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
if (digest(readFileSync('backend/contract/plugin.wit')) !== lock.sha256) throw new Error('WIT snapshot checksum mismatch');
if (process.env.AIO_PLATFORM && digest(readFileSync(`${process.env.AIO_PLATFORM}/lib/plugin/contract/wit/plugin.wit`)) !== lock.sha256) throw new Error('Platform contract changed; regenerate bindings and migrate the plugin together');
