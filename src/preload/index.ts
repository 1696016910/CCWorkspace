import { contextBridge, ipcRenderer } from 'electron'
import type {
  ChatMessage,
  ContextUsageDetail,
  EnvEntry,
  MarketplaceInfo,
  PluginInfo,
  SessionSummary,
  SkillInfo,
  SlashCommandInfo,
  Workspace
} from '../shared/types'

/**
 * 渲染进程可用的 API 白名单。
 * 渲染层不接触 fs / child_process / sqlite，全部经此转发。
 */

/**
 * Electron 会把主进程异常包成 "Error invoking remote method 'x': Error: 真实信息"。
 * 这里剥掉前缀，让界面拿到可读的业务错误。
 */
function unwrap<T>(promise: Promise<T>): Promise<T> {
  return promise.catch((e: unknown) => {
    const raw = e instanceof Error ? e.message : String(e)
    const cleaned = raw
      .replace(/^Error invoking remote method '[^']+':\s*/i, '')
      .replace(/^Error:\s*/i, '')
    throw new Error(cleaned || '操作失败')
  })
}

const api = {
  window: {
    minimize: (): Promise<void> => unwrap(ipcRenderer.invoke('window:minimize')),
    maximize: (): Promise<void> => unwrap(ipcRenderer.invoke('window:maximize')),
    close: (): Promise<void> => unwrap(ipcRenderer.invoke('window:close')),
  },

  dialog: {
    confirm: (message: string): Promise<boolean> => unwrap(ipcRenderer.invoke('dialog:confirm', message)),
    pickFiles: (): Promise<string[]> => unwrap(ipcRenderer.invoke('dialog:pickFiles'))
  },

  theme: {
    get: (): Promise<'light' | 'dark'> => unwrap(ipcRenderer.invoke('theme:get')),
    set: (theme: 'light' | 'dark'): Promise<void> => unwrap(ipcRenderer.invoke('theme:set', theme))
  },

  file: {
    open: (path: string): Promise<void> => unwrap(ipcRenderer.invoke('file:open', path)),
    savePaste: (name: string, base64: string): Promise<string> =>
      unwrap(ipcRenderer.invoke('file:savePaste', name, base64)),
    read: (
      path: string
    ): Promise<{ path: string; name: string; ext: string; size: number; content: string }> =>
      unwrap(ipcRenderer.invoke('file:read', path))
  },

  workspace: {
    list: (): Promise<Workspace[]> => unwrap(ipcRenderer.invoke('workspace:list')),
    add: (dir: string): Promise<Workspace[]> => unwrap(ipcRenderer.invoke('workspace:add', dir)),
    remove: (id: string): Promise<Workspace[]> => unwrap(ipcRenderer.invoke('workspace:remove', id)),
    getDefault: (): Promise<string | null> => unwrap(ipcRenderer.invoke('workspace:getDefault')),
    setDefault: (id: string | null): Promise<void> => unwrap(ipcRenderer.invoke('workspace:setDefault', id)),
    rename: (id: string, name: string): Promise<Workspace[]> =>
      unwrap(ipcRenderer.invoke('workspace:rename', id, name)),
    pick: (): Promise<string | null> => unwrap(ipcRenderer.invoke('workspace:pick'))
  },

  session: {
    list: (): Promise<SessionSummary[]> => unwrap(ipcRenderer.invoke('session:list')),
    listDeleted: (): Promise<SessionSummary[]> => unwrap(ipcRenderer.invoke('session:listDeleted')),
    messages: (id: string, limit?: number): Promise<ChatMessage[]> =>
      unwrap(ipcRenderer.invoke('session:messages', id, limit)),
    messageCount: (id: string): Promise<number> =>
      unwrap(ipcRenderer.invoke('session:messageCount', id)),
    create: (workspaceId: string | null): Promise<SessionSummary> =>
      unwrap(ipcRenderer.invoke('session:create', workspaceId)),
    rename: (id: string, title: string): Promise<void> =>
      unwrap(ipcRenderer.invoke('session:rename', id, title)),
    remove: (id: string): Promise<void> => unwrap(ipcRenderer.invoke('session:remove', id)),
    restore: (id: string): Promise<void> => unwrap(ipcRenderer.invoke('session:restore', id)),
    purge: (id: string): Promise<void> => unwrap(ipcRenderer.invoke('session:purge', id)),
    duplicate: (id: string): Promise<SessionSummary> => unwrap(ipcRenderer.invoke('session:duplicate', id))
  },

  chat: {
    send: (sessionId: string, text: string): Promise<void> =>
      unwrap(ipcRenderer.invoke('chat:send', sessionId, text)),
    interrupt: (): Promise<void> => unwrap(ipcRenderer.invoke('chat:interrupt')),
    onEvent: (handler: (event: unknown) => void): (() => void) => {
      const listener = (_e: unknown, payload: unknown): void => handler(payload)
      ipcRenderer.on('chat:event', listener)
      return () => ipcRenderer.removeListener('chat:event', listener)
    }
  },

  context: {
    /** 上下文用量明细（token 数与百分比） */
    usage: (sessionId: string): Promise<ContextUsageDetail> =>
      unwrap(ipcRenderer.invoke('context:usage', sessionId)),
    /** 压缩当前会话的上下文 */
    compact: (sessionId: string): Promise<void> =>
      unwrap(ipcRenderer.invoke('context:compact', sessionId))
  },

  commands: {
    /** 会话可用的斜杠命令与 Skill */
    list: (sessionId: string): Promise<SlashCommandInfo[]> =>
      unwrap(ipcRenderer.invoke('commands:list', sessionId))
  },

  config: {
    readEnv: (): Promise<EnvEntry[]> => unwrap(ipcRenderer.invoke('config:readEnv')),
    saveEnv: (entries: EnvEntry[]): Promise<void> => unwrap(ipcRenderer.invoke('config:saveEnv', entries)),
    readPermissions: (): Promise<{ defaultMode: string; allow: string[]; deny: string[] }> =>
      unwrap(ipcRenderer.invoke('config:readPermissions')),
    savePermissions: (p: { defaultMode: string; allow: string[]; deny: string[] }): Promise<void> =>
      unwrap(ipcRenderer.invoke('config:savePermissions', p)),
    readModels: (): Promise<Record<string, string>> => unwrap(ipcRenderer.invoke('config:readModels')),
    saveModels: (m: Record<string, string>): Promise<void> =>
      unwrap(ipcRenderer.invoke('config:saveModels', m)),
    testConnection: (): Promise<{ ok: boolean; message: string; ms: number }> =>
      unwrap(ipcRenderer.invoke('config:testConnection'))
  },

  skill: {
    list: (): Promise<SkillInfo[]> => unwrap(ipcRenderer.invoke('skill:list')),
    toggle: (name: string, enabled: boolean): Promise<SkillInfo[]> =>
      unwrap(ipcRenderer.invoke('skill:toggle', name, enabled))
  },

  plugin: {
    list: (): Promise<PluginInfo[]> => unwrap(ipcRenderer.invoke('plugin:list')),
    toggle: (id: string, enabled: boolean): Promise<PluginInfo[]> =>
      unwrap(ipcRenderer.invoke('plugin:toggle', id, enabled)),
    install: (id: string): Promise<void> => unwrap(ipcRenderer.invoke('plugin:install', id)),
    uninstall: (id: string): Promise<void> => unwrap(ipcRenderer.invoke('plugin:uninstall', id))
  },

  marketplace: {
    list: (): Promise<MarketplaceInfo[]> => unwrap(ipcRenderer.invoke('marketplace:list')),
    add: (name: string, source: string): Promise<MarketplaceInfo[]> =>
      unwrap(ipcRenderer.invoke('marketplace:add', name, source)),
    remove: (name: string): Promise<MarketplaceInfo[]> =>
      unwrap(ipcRenderer.invoke('marketplace:remove', name))
  }
}

contextBridge.exposeInMainWorld('api', api)

export type Api = typeof api
