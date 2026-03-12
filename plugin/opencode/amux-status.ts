// amux-status v2.1
import { Plugin as OCPlugin } from '@opencode-ai/plugin';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

const DEBUG = process.env.AMUX_STATUS_DEBUG === '1';

type Status = 'idle' | 'busy' | 'errored' | 'awaiting_input';

function getStatusDir(): string {
    const xdgState =
        process.env.XDG_STATE_HOME ||
        path.join(os.homedir(), '.local', 'state');
    return path.join(xdgState, 'amux', 'opencode');
}

function getStatusFilePath(paneId: string): string {
    return path.join(getStatusDir(), `${paneId}.json`);
}

function writeStatus(paneId: string, status: Status) {
    const dir = getStatusDir();
    fs.mkdirSync(dir, { recursive: true });
    const payload = JSON.stringify({
        status,
        pid: process.pid,
        ts: Math.floor(Date.now() / 1000),
    });
    fs.writeFileSync(getStatusFilePath(paneId), payload);
}

function removeStatus(paneId: string) {
    const filePath = getStatusFilePath(paneId);
    try {
        fs.unlinkSync(filePath);
    } catch (_) {}
}

const Plugin: OCPlugin = async ({ $, client }) => {
    const log = (
        level: 'debug' | 'info' | 'error' | 'warn',
        message: string,
        extra: Record<string, unknown> = {}
    ): void => {
        if (DEBUG) {
            client.app.log({
                body: {
                    service: 'amux',
                    level,
                    message,
                    extra,
                },
            });
        }
    };

    const paneId = process.env.TMUX_PANE;
    if (!paneId) return {};

    const cleanup = () => removeStatus(paneId);
    process.on('exit', cleanup);
    process.on('SIGINT', () => {
        cleanup();
        process.exit(130);
    });
    process.on('SIGTERM', () => {
        cleanup();
        process.exit(143);
    });

    log('info', 'plugin initialized', { paneId });

    return {
        event: async ({ event }) => {
            log('debug', 'event received', {
                type: event.type,
                properties: event.properties,
            });

            if (event.type === 'session.status') {
                const status =
                    event.properties.status.type === 'busy' ? 'busy' : 'idle';
                writeStatus(paneId, status);
                log('info', 'status written', { paneId, status });
            }

            if (event.type === 'session.error') {
                writeStatus(paneId, 'errored');
                log('info', 'error status written', { paneId });
            }

            // @ts-expect-error Need to check official types against docs
            if (event.type === 'question.asked') {
                writeStatus(paneId, 'awaiting_input');
                log('info', 'awaiting_input status written (question asked)', {
                    paneId,
                });
            }

            // @ts-expect-error Need to check official types against docs
            if (event.type === 'question.replied') {
                writeStatus(paneId, 'busy');
                log('info', 'busy status written (question replied)', {
                    paneId,
                });
            }
        },

        'permission.ask': async (_permission, output) => {
            if (output.status === 'ask') {
                writeStatus(paneId, 'awaiting_input');
                log('info', 'awaiting_input status written', { paneId });
            }
        },
    };
};

module.exports = { Plugin };
