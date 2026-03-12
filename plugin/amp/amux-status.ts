// @i-know-the-amp-plugin-api-is-wip-and-very-experimental-right-now
// amux-status v1.0
import type { PluginAPI } from '@ampcode/plugin'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

type Status = 'idle' | 'busy' | 'errored'

function getStatusDir(): string {
    const xdgState =
        process.env.XDG_STATE_HOME ||
        path.join(os.homedir(), '.local', 'state')
    return path.join(xdgState, 'amux', 'amp')
}

function getStatusFilePath(paneId: string): string {
    return path.join(getStatusDir(), `${paneId}.json`)
}

function writeStatus(paneId: string, status: Status) {
    const dir = getStatusDir()
    fs.mkdirSync(dir, { recursive: true })
    const payload = JSON.stringify({
        status,
        pid: process.pid,
        ts: Math.floor(Date.now() / 1000),
    })
    fs.writeFileSync(getStatusFilePath(paneId), payload)
}

function removeStatus(paneId: string) {
    const filePath = getStatusFilePath(paneId)
    try {
        fs.unlinkSync(filePath)
    } catch (_) {}
}

export default function (amp: PluginAPI) {
    const paneId = process.env.TMUX_PANE
    if (!paneId) return

    const cleanup = () => removeStatus(paneId)
    process.on('exit', cleanup)
    process.on('SIGINT', () => {
        cleanup()
        process.exit(130)
    })
    process.on('SIGTERM', () => {
        cleanup()
        process.exit(143)
    })

    amp.logger.log('[amux] plugin initialized', { paneId })

    amp.on('agent.start', (_event, ctx) => {
        writeStatus(paneId, 'busy')
        ctx.logger.log('[amux] status: busy (agent.start)')
        return {};
    })

    amp.on('agent.end', (event, ctx) => {
        const status: Status = event.status === 'error' ? 'errored' : 'idle'
        writeStatus(paneId, status)
        ctx.logger.log(`[amux] status: ${status} (agent.end)`)
    })
}
