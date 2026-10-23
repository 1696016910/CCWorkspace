import { contextBridge, ipcRenderer } from 'electron'
import type {
  ChatMessage,
  EnvEntry,
  MarketplaceInfo,
  PluginInfo,
  SessionSummary,
  SkillInfo,
  Workspace
} from '../shared/types'

/**
 * 渲染进程可用的 API 白名单。
 * 渲染层不接触 fs / child_process / sqlite，全部经此转发。
 */
const api = {
  ping: (): Promise<string> => ipcRenderer.invoke('app:ping'),

  workspace: {
    list: (): Promise<Workspace[]> => ipcRenderer.invoke('workspace:list'),
    add: (dir: string): Promise<Workspace[]> => ipcRenderer.invoke('workspace:add', dir),
    remove: (id: string): Promise<Workspace[]> => ipcRenderer.invoke('workspace:remove', id),
    getDefault: (): Promise<string | null> => ipcRenderer.invoke('workspace:getDefault'),
    setDefault: (id: string | null): Promise<void> => ipcRenderer.invoke('workspace:setDefault', id),
    pick: (): Promise<string | null> => ipcRenderer.invoke('workspace:pick')
  },

  session: {
    list: (): Promise<SessionSummary[]> => ipcRenderer.invoke('session:list'),
    listDeleted: (): Promise<SessionSummary[]> => ipcRenderer.invoke('session:listDeleted'),
    messages: (id: string): Promise<ChatMessage[]> => ipcRenderer.invoke('session:messages', id),
    create: (workspaceId: string | null): Promise<SessionSummary> =>
      ipcRenderer.invoke('session:create', workspaceId),
    rename: (id: string, title: string): Promise<void> =>
      ipcRenderer.invoke('session:rename', id, title),
    remove: (id: string): Promise<void> => ipcRenderer.invoke('session:remove', id),
    restore: (id: string): Promise<void> => ipcRenderer.invoke('session:restore', id),
    duplicate: (id: string): Promise<SessionSummary> => ipcRenderer.invoke('session:duplicate', id)
  },

  chat: {
    send: (sessionId: string, text: string): Promise<void> =>
      ipcRenderer.invoke('chat:send', sessionId, text),
    interrupt: (): Promise<void> => ipcRenderer.invoke('chat:interrupt'),
    onEvent: (handler: (event: unknown) => void): (() => void) => {
      const listener = (_e: unknown, payload: unknown): void => handler(payload)
      ipcRenderer.on('chat:event', listener)
      return () => ipcRenderer.removeListener('chat:event', listener)
    }
  },

  config: {
    readEnv: (): Promise<EnvEntry[]> => ipcRenderer.invoke('config:readEnv'),
    saveEnv: (entries: EnvEntry[]): Promise<void> => ipcRenderer.invoke('config:saveEnv', entries),
    readPermissions: (): Promise<{ defaultMode: string; allow: string[]; deny: string[] }> =>
      ipcRenderer.invoke('config:readPermissions'),
    savePermissions: (p: { defaultMode: string; allow: string[]; deny: string[] }): Promise<void> =>
      ipcRenderer.invoke('config:savePermissions', p),
    readModels: (): Promise<Record<string, string>> => ipcRenderer.invoke('config:readModels'),
    saveModels: (m: Record<string, string>): Promise<void> =>
      ipcRenderer.invoke('config:saveModels', m),
    testConnection: (): Promise<{ ok: boolean; message: string; ms: number }> =>
      ipcRenderer.invoke('config:testConnection')
  },

  skill: {
    list: (): Promise<SkillInfo[]> => ipcRenderer.invoke('skill:list'),
    toggle: (name: string, enabled: boolean): Promise<SkillInfo[]> =>
      ipcRenderer.invoke('skill:toggle', name, enabled)
  },

  plugin: {
    list: (): Promise<PluginInfo[]> => ipcRenderer.invoke('plugin:list'),
    toggle: (id: string, enabled: boolean): Promise<PluginInfo[]> =>
      ipcRenderer.invoke('plugin:toggle', id, enabled),
    install: (id: string): Promise<void> => ipcRenderer.invoke('plugin:install', id),
    uninstall: (id: string): Promise<void> => ipcRenderer.invoke('plugin:uninstall', id)
  },

  marketplace: {
    list: (): Promise<MarketplaceInfo[]> => ipcRenderer.invoke('marketplace:list'),
    add: (name: string, source: string): Promise<MarketplaceInfo[]> =>
      ipcRenderer.invoke('marketplace:add', name, source),
    remove: (name: string): Promise<MarketplaceInfo[]> =>
      ipcRenderer.invoke('marketplace:remove', name)
  }
}

contextBridge.exposeInMainWorld('api', api)

export type Api = typeof api
