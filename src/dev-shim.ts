/**
 * 浏览器里跑前端用的假 IPC。
 *
 * Tauri 的 WebView 没有远程调试端口，Playwright 接不进去；在普通浏览器里打开
 * Vite 端口时又缺少 `window.__TAURI_INTERNALS__`，应用起不来，布局与交互没法自测。
 *
 * 只在 URL 带 `?shim=1` 时生效，正式构建不会被引用。
 */

type Args = Record<string, unknown> | undefined

const WORKSPACES = [
  { id: 'w1', name: '测试ttt', path: 'C:/Users/MasterY/Downloads/ttt', addedAt: '', pinnedAt: null }
]

const SESSIONS = [
  {
    id: 's1',
    workspaceId: 'w1',
    title: '111',
    updatedAt: new Date().toISOString(),
    messageCount: 2,
    running: false,
    renamed: true,
    contextPercent: 3,
    contextTokens: 3000,
    contextMax: 100000,
    pinnedAt: null
  },
  {
    id: 's2',
    workspaceId: 'w1',
    title: '222',
    updatedAt: new Date(Date.now() - 3600_000).toISOString(),
    messageCount: 0,
    running: false,
    renamed: false,
    contextPercent: 0,
    contextTokens: 0,
    contextMax: 0,
    pinnedAt: null
  }
]

const MESSAGES: Record<string, unknown[]> = {
  s1: [
    {
      id: 'm1',
      role: 'user',
      text: '你好 给我写一个一万字的短文 主题是 AI 改变生活',
      timestamp: new Date().toISOString()
    },
    {
      id: 'm2',
      role: 'assistant',
      text: '这是一篇约 1.3 万字的议论文。参考 `/src/utils/markdown.ts` 与 `C:\\Users\\a\\b.md`，另外处理 Word/Excel/PPT/PDF 文件。',
      timestamp: new Date().toISOString()
    }
  ],
  s2: []
}

/** 假事件总线：chat_send 之后按脚本推流式事件 */
const listeners = new Map<string, number>()
const callbacks = new Map<number, (p: unknown) => void>()
let cbSeq = 1

function emit(event: string, payload: unknown): void {
  for (const [key, id] of listeners) {
    if (key !== event) continue
    callbacks.get(id)?.({ event, id, payload })
  }
}

let runSeq = 1
const activeRuns = new Map<string, number>()
const timers = new Map<string, ReturnType<typeof setInterval>>()

export function install(): void {
  const invoke = async (cmd: string, args: Args): Promise<unknown> => {
    switch (cmd) {
      case 'workspace_list':
        return WORKSPACES
      case 'session_list':
        return SESSIONS
      case 'session_list_deleted':
        return []
      case 'session_messages':
        return MESSAGES[(args?.id as string) ?? 's1'] ?? []
      case 'session_message_count':
        return 2
      case 'cli_meta':
        return {
          slashCommands: [
            'clear',
            'compact',
            'model',
            'java-style',
            'frontend-style',
            'opsx:apply',
            'opsx:archive',
            'pua:yes',
            'pua:ding'
          ],
          skills: ['java-style', 'frontend-style'],
          agents: [],
          plugins: ['pua']
        }
      case 'skill_list':
        return [
          { name: 'java-style', description: 'Java 编码规范', source: 'user', extraFiles: 0 },
          { name: 'frontend-style', description: 'Vue 3 规范', source: 'project', extraFiles: 2 }
        ]
      case 'rule_list':
        return [
          { name: 'ai-coding-constitution', description: 'AI 行为总宪法', source: 'user' },
          { name: 'workspace-rules', description: '工作区通用规则', source: 'user' }
        ]
      case 'workspace_list_dir':
        return [
          { name: '.ruff_cache', path: 'C:/tmp/.ruff_cache', isDir: true },
          { name: 'docs', path: 'C:/tmp/docs', isDir: true },
          { name: 'src', path: 'C:/tmp/src', isDir: true },
          { name: 'CLAUDE.md', path: 'C:/tmp/CLAUDE.md', isDir: false, size: 1229 },
          { name: 'multiplication_table.py', path: 'C:/tmp/multiplication_table.py', isDir: false, size: 1843 }
        ]
      case 'rule_read':
        return ['---', 'description: AI 行为总宪法', '---', '', '# AI Coding Constitution', '', '先理解再修改。', ''].join(String.fromCharCode(10))
      case 'skill_read':
        return ['---', 'name: java-style', 'description: Java 编码规范', '---', '', '# Java', '', '命名用驼峰。', ''].join(String.fromCharCode(10))
      case 'command_list':
        return []
      case 'file_check':
        return ((args?.paths as string[]) ?? []).map((p) => !p.includes('b.md'))
      case 'file_read':
        return {
          name: 'markdown.ts',
          path: String(args?.path),
          ext: 'ts',
          text: '// 假内容\nconsole.log(1)\n',
          size: 20
        }
      case 'file_image':
        return {
          name: 'shot.png',
          mediaType: 'image/png',
          // 1x1 透明 PNG
          data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
          preview:
            'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
          size: 68
        }
      case 'app_version':
        return '0.2.1'
      case 'config_read_permissions':
        return { defaultMode: 'default', allow: ['Bash(git diff:*)'], deny: ['Read(./.env)'] }
      case 'config_read_env':
        return [{ key: 'ANTHROPIC_BASE_URL', value: 'http://x' }]
      case 'config_read_models':
        return {}
      case 'config_health':
        return null
      case 'chat_send': {
        const sid = String(args?.sessionId)
        const runId = runSeq++
        activeRuns.set(sid, runId)
        // 先回 start，再分几拍推文本
        setTimeout(() => emit('chat:event', { type: 'start', sessionId: sid, runId }), 0)
        const full = '这是一段用于验证界面的假回复。它会被分批推送，用来检查流式渲染、停止按钮与侧栏状态标记。'
        let i = 0
        const timer = setInterval(() => {
          if (i >= full.length) {
            clearInterval(timer)
            activeRuns.delete(sid)
            emit('chat:event', {
              type: 'done',
              sessionId: sid,
              runId,
              message: { id: `m-${runId}`, role: 'assistant', text: full }
            })
            return
          }
          emit('chat:event', { type: 'text', sessionId: sid, runId, delta: full.slice(i, i + 6) })
          i += 6
        }, 80)
        timers.set(sid, timer)
        return runId
      }
      case 'chat_interrupt': {
        const sid = String(args?.sessionId)
        const runId = activeRuns.get(sid)
        if (runId === undefined) return null
        activeRuns.delete(sid)
        if (timers.has(sid)) {
          clearInterval(timers.get(sid))
          timers.delete(sid)
        }
        // 真实后端会推 done 并带 interrupted 标记
        setTimeout(
          () =>
            emit('chat:event', {
              type: 'done',
              sessionId: sid,
              runId,
              interrupted: true,
              message: { id: `m-${runId}`, role: 'assistant', text: '部分内容（已停止生成）' }
            }),
          60
        )
        return null
      }
      case 'context_compact':
        return null
      default:
        return null
    }
  }

  // 与 @tauri-apps/api 的调用约定一致
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: (p: unknown) => void) => {
      const id = cbSeq++
      callbacks.set(id, cb)
      return id
    },
    unregisterCallback: (id: number) => {
      callbacks.delete(id)
    },
    // @tauri-apps/api/event 用这几个
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } }
  }
  ;(window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.delete(`${event}:${id}`)
      callbacks.delete(id)
    }
  }

  // event.listen 会调 plugin:event|listen
  const origInvoke = invoke
  ;(window as unknown as Record<string, { invoke: typeof invoke }>).__TAURI_INTERNALS__.invoke = async (
    cmd: string,
    args: Args
  ) => {
    if (cmd === 'plugin:event|listen') {
      const ev = String(args?.event)
      const handler = Number(args?.handler)
      listeners.set(`${ev}:${handler}`, handler)
      return handler
    }
    if (cmd === 'plugin:event|unlisten') return null
    if (cmd.startsWith('plugin:dialog|')) {
      // 浏览器里没有系统对话框，直接返回一张假图片的 base64
      if (cmd === 'plugin:dialog|open') return 'C:/tmp/shot.png'
      return null
    }
    return origInvoke(cmd, args)
  }
}

if (typeof window !== 'undefined' && location.search.includes('shim=1')) {
  install()
}
