import { app, BrowserWindow, dialog, ipcMain, Menu, screen, shell } from 'electron'
import { join } from 'node:path'
import { readFileSync, writeFileSync } from 'node:fs'
import { initDb, closeDb } from './db'
import {
  addWorkspace,
  getDefaultWorkspaceId,
  listWorkspaces,
  removeWorkspace,
  renameWorkspace,
  setDefaultWorkspaceId,
  getTheme,
  setTheme
} from './services/workspace'
import {
  createSession,
  deleteSession,
  duplicateSession,
  listDeletedSessions,
  listMessages,
  countMessages,
  listSessions,
  renameSession,
  restoreSession,
  purgeSession
} from './services/session'
import { assertCanSend, compact, getContextUsage, interrupt, listCommands, refreshContext, send } from './services/chat'
import {
  getLastReadError,
  readEnv,
  readModels,
  readPermissions,
  saveEnv,
  saveModels,
  savePermissions,
  testConnection
} from './services/claudeSettings'
import { listSkills, slashCandidates, toggleSkill } from './services/skills'
import {
  addMarketplace,
  installPlugin,
  listMarketplaces,
  listPlugins,
  removeMarketplace,
  togglePlugin,
  uninstallPlugin
} from './services/plugins'
import type { EnvEntry } from '../shared/types'

let mainWindow: BrowserWindow | null = null

/**
 * 记住窗口位置与大小。
 * 存在 userData 下的独立文件里，避免与数据库耦合；
 * 读取失败时回落到默认尺寸，不影响启动。
 */
function windowStatePath(): string {
  return join(app.getPath('userData'), 'window-state.json')
}

interface WindowState {
  x?: number
  y?: number
  width: number
  height: number
  maximized?: boolean
}

function readWindowState(): WindowState {
  const fallback: WindowState = { width: 1440, height: 900 }
  try {
    const raw = readFileSync(windowStatePath(), 'utf-8')
    const parsed = JSON.parse(raw) as WindowState
    if (typeof parsed.width !== 'number' || typeof parsed.height !== 'number') return fallback
    return parsed
  } catch {
    return fallback
  }
}

function saveWindowState(): void {
  if (!mainWindow) return
  try {
    const maximized = mainWindow.isMaximized()
    // 最大化时记录还原后的尺寸，避免下次启动被钳在最大化状态
    const bounds = maximized ? mainWindow.getNormalBounds() : mainWindow.getBounds()
    writeFileSync(
      windowStatePath(),
      JSON.stringify({ ...bounds, maximized }, null, 2),
      'utf-8'
    )
  } catch {
    // 写失败不影响退出
  }
}

/**
 * 校验保存的位置是否仍落在某块屏幕上。
 * 外接显示器拔掉后，旧坐标会落在屏幕外，窗口将完全不可见。
 */
function sanitizeBounds(state: WindowState): WindowState {
  if (state.x === undefined || state.y === undefined) return state
  try {
    const displays = screen.getAllDisplays()
    const visible = displays.some((d) => {
      const b = d.workArea
      return (
        state.x! >= b.x - 50 &&
        state.y! >= b.y - 50 &&
        state.x! < b.x + b.width &&
        state.y! < b.y + b.height
      )
    })
    if (visible) return state
  } catch {
    // 拿不到屏幕信息时按原值处理
  }
  return { width: state.width, height: state.height, maximized: state.maximized }
}

function createWindow(): void {
  const state = sanitizeBounds(readWindowState())

  mainWindow = new BrowserWindow({
    x: state.x,
    y: state.y,
    width: state.width,
    height: state.height,
    minWidth: 960,
    minHeight: 600,
    show: false,
    backgroundColor: '#ffffff',
    // 使用系统原生标题栏与窗口按钮
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, '../preload/index.mjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  })

  if (state.maximized) mainWindow.maximize()

  mainWindow.on('ready-to-show', () => mainWindow?.show())

  let saveTimer: NodeJS.Timeout | null = null
  const scheduleSave = (): void => {
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(saveWindowState, 400)
  }
  mainWindow.on('resize', scheduleSave)
  mainWindow.on('move', scheduleSave)
  mainWindow.on('close', saveWindowState)

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url)
    return { action: 'deny' }
  })

  if (process.env['ELECTRON_RENDERER_URL']) {
    void mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    void mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

/** 统一把异常转成渲染层可读的错误，避免 IPC 调用直接 reject 而丢信息 */
function handle(channel: string, fn: (...args: never[]) => unknown): void {
  ipcMain.handle(channel, async (_event, ...args) => {
    try {
      return await (fn as (...a: unknown[]) => unknown)(...args)
    } catch (e) {
      throw new Error(e instanceof Error ? e.message : String(e))
    }
  })
}

function registerIpc(): void {
  /* 主题 */
  handle('theme:get', () => getTheme())
  handle('theme:set', (theme: 'light' | 'dark') => setTheme(theme))

  /* 打开文件或目录（用于点击生成的文件） */
  handle('file:open', async (path: string) => {
    const err = await shell.openPath(path)
    if (err) throw new Error(err)
  })

  /**
   * 读取文件内容供应用内预览。
   * 只读不写，并对体积与二进制做限制，避免把大文件读进渲染进程。
   */
  handle('file:read', async (path: string) => {
    const { readFile, stat } = await import('node:fs/promises')
    const { extname, basename } = await import('node:path')

    let info
    try {
      info = await stat(path)
    } catch {
      throw new Error('文件不存在或无法访问')
    }
    if (!info.isFile()) throw new Error('该路径不是文件')

    const MAX = 2 * 1024 * 1024
    if (info.size > MAX) {
      throw new Error(`文件过大（${Math.round(info.size / 1024)} KB），暂不支持预览`)
    }

    const ext = extname(path).toLowerCase()
    const binaryExts = new Set([
      '.png', '.jpg', '.jpeg', '.gif', '.webp', '.ico', '.bmp',
      '.pdf', '.zip', '.gz', '.7z', '.rar', '.exe', '.dll', '.so',
      '.woff', '.woff2', '.ttf', '.mp3', '.mp4', '.mov'
    ])
    if (binaryExts.has(ext)) {
      throw new Error('这是二进制文件，无法以文本方式预览')
    }

    // 先按字节读入，再判断编码：Windows 上的中文文档常是 GBK
    let buf: Buffer
    try {
      buf = await readFile(path)
    } catch {
      throw new Error('读取失败：文件无法访问')
    }

    // UTF-8 BOM
    if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) {
      buf = buf.subarray(3)
    }

    let content: string
    try {
      content = new TextDecoder('utf-8', { fatal: true }).decode(buf)
    } catch {
      // UTF-8 解不开时按 GBK 处理（中文 Windows 的常见编码）
      try {
        content = new TextDecoder('gbk').decode(buf)
      } catch {
        throw new Error('读取失败：文件编码无法识别')
      }
    }

    return {
      path,
      name: basename(path),
      ext,
      size: info.size,
      content
    }
  })

  /**
   * 保存粘贴进来的图片或文件到应用数据目录。
   * 渲染进程只有 base64，落盘后才能把路径交给模型。
   */
  handle('file:savePaste', async (name: string, base64: string) => {
    const { mkdir, writeFile } = await import('node:fs/promises')
    const dir = join(app.getPath('userData'), 'attachments')
    await mkdir(dir, { recursive: true })
    const safe = name.replace(/[^\w.\-]/g, '_')
    const target = join(dir, `${Date.now()}-${safe}`)
    await writeFile(target, Buffer.from(base64, 'base64'))
    return target
  })

  /* 窗口控制 */
  handle('window:minimize', () => mainWindow?.minimize())
  handle('window:maximize', () => {
    if (!mainWindow) return
    if (mainWindow.isMaximized()) mainWindow.unmaximize()
    else mainWindow.maximize()
  })
  handle('window:close', () => mainWindow?.close())

  /* 原生确认对话框（渲染层 window.confirm 在 Electron 中不可用） */
  handle('dialog:confirm', async (message: string) => {
    if (!mainWindow) return false
    const { response } = await dialog.showMessageBox(mainWindow, {
      type: 'question',
      buttons: ['取消', '确定'],
      defaultId: 1,
      cancelId: 0,
      message
    })
    return response === 1
  })

  /* 打开文件选择器，返回选中的文件路径 */
  handle('dialog:pickFiles', async () => {
    if (!mainWindow) return []
    const result = await dialog.showOpenDialog(mainWindow, {
      properties: ['openFile', 'multiSelections'],
      title: '选择文件'
    })
    return result.canceled ? [] : result.filePaths
  })

  /* 工作区 */
  handle('workspace:list', () => listWorkspaces())
  handle('workspace:add', (dir: string) => addWorkspace(dir))
  handle('workspace:remove', (id: string) => removeWorkspace(id))
  handle('workspace:getDefault', () => getDefaultWorkspaceId())
  handle('workspace:setDefault', (id: string | null) => setDefaultWorkspaceId(id))
  handle('workspace:rename', (id: string, name: string) => renameWorkspace(id, name))
  handle('workspace:pick', async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory'],
      title: '选择项目目录'
    })
    return result.canceled ? null : (result.filePaths[0] ?? null)
  })

  /* 会话 */
  handle('session:list', () => listSessions())
  handle('session:listDeleted', () => listDeletedSessions())
  handle('session:messages', (id: string, limit?: number) => listMessages(id, limit))
  handle('session:messageCount', (id: string) => countMessages(id))
  handle('session:create', (workspaceId: string | null) => createSession(workspaceId))
  handle('session:rename', (id: string, title: string) => renameSession(id, title))
  handle('session:remove', (id: string) => deleteSession(id))
  handle('session:restore', (id: string) => restoreSession(id))
  handle('session:duplicate', (id: string) => duplicateSession(id))
  handle('session:purge', (id: string) => purgeSession(id))

  /* 对话 */
  handle('chat:send', async (sessionId: string, text: string) => {
    const sessions = listSessions()
    const session = sessions.find((s) => s.id === sessionId)
    if (!session) throw new Error('会话不存在')
    const workspace = listWorkspaces().find((w) => w.id === session.workspaceId)
    if (!workspace) throw new Error('会话所属工作区已被移除')

    const skills = (await listSkills(workspace.path)).filter((s) => s.enabled).map((s) => s.name)

    // 同步前置校验：这类错误要立即反馈给调用方
    assertCanSend(text)

    // 不 await：回复通过 chat:event 流式推送，IPC 立即返回，
    // 否则渲染层要等整轮结束，输入框会一直处于禁用状态
    void send(sessionId, text, workspace.path, skills).catch((e: unknown) => {
      BrowserWindow.getAllWindows().forEach((w) =>
        w.webContents.send('chat:event', {
          type: 'error',
          sessionId,
          message: e instanceof Error ? e.message : String(e)
        })
      )
    })
  })
  handle('chat:interrupt', () => interrupt())

  /* 上下文与命令 */
  handle('context:usage', async (sessionId: string) => {
    const session = listSessions().find((s) => s.id === sessionId)
    const workspace = session
      ? listWorkspaces().find((w) => w.id === session.workspaceId)
      : undefined
    if (!workspace) return getContextUsage(sessionId)
    const skills = (await listSkills(workspace.path)).filter((s) => s.enabled).map((s) => s.name)
    return refreshContext(sessionId, workspace.path, skills)
  })
  handle('context:compact', async (sessionId: string) => {
    const session = listSessions().find((s) => s.id === sessionId)
    if (!session) throw new Error('会话不存在')
    const workspace = listWorkspaces().find((w) => w.id === session.workspaceId)
    if (!workspace) throw new Error('会话所属工作区已被移除')
    const skills = (await listSkills(workspace.path)).filter((s) => s.enabled).map((s) => s.name)
    await compact(sessionId, workspace.path, skills)
  })
  handle('commands:list', async (sessionId: string) => {
    // 优先用 SDK 在首轮 init 中给出的完整清单；没有则退回本地扫描
    const cached = await listCommands()
    if (cached.length > 0) return cached

    const session = listSessions().find((s) => s.id === sessionId)
    const workspace = session
      ? listWorkspaces().find((w) => w.id === session.workspaceId)
      : undefined
    return slashCandidates(workspace?.path ?? null)
  })

  /* 配置 */
  handle('config:readEnv', () => readEnv())
  handle('config:health', () => ({ warning: getLastReadError() }))
  handle('config:saveEnv', (entries: EnvEntry[]) => saveEnv(entries))
  handle('config:readPermissions', () => readPermissions())
  handle('config:savePermissions', (p: { defaultMode: string; allow: string[]; deny: string[] }) =>
    savePermissions(p)
  )
  handle('config:readModels', () => readModels())
  handle('config:saveModels', (m: Record<string, string>) => saveModels(m))
  handle('config:testConnection', () => testConnection())

  /* Skill */
  handle('skill:list', async () => {
    const def = getDefaultWorkspaceId()
    const ws = listWorkspaces().find((w) => w.id === def)
    return listSkills(ws?.path ?? null)
  })
  handle('skill:toggle', async (name: string, enabled: boolean) => {
    await toggleSkill(name, enabled)
    const def = getDefaultWorkspaceId()
    const ws = listWorkspaces().find((w) => w.id === def)
    return listSkills(ws?.path ?? null)
  })

  /* 插件与仓库 */
  handle('plugin:list', () => listPlugins())
  handle('plugin:toggle', async (id: string, enabled: boolean) => {
    await togglePlugin(id, enabled)
    return listPlugins()
  })
  handle('plugin:install', (id: string) => installPlugin(id))
  handle('plugin:uninstall', (id: string) => uninstallPlugin(id))
  handle('marketplace:list', () => listMarketplaces())
  handle('marketplace:add', async (name: string, repo: string) => {
    await addMarketplace(name, repo)
    return listMarketplaces()
  })
  handle('marketplace:remove', async (name: string) => {
    await removeMarketplace(name)
    return listMarketplaces()
  })
}

/**
 * 单实例锁。
 *
 * 两个实例会同时打开同一个 SQLite 文件，写操作互相阻塞甚至损坏，
 * 因此第二个实例直接把已有窗口唤到前台后退出。
 */
const gotLock = app.requestSingleInstanceLock()
if (!gotLock) {
  app.quit()
} else {
  app.on('second-instance', () => {
    if (!mainWindow) return
    if (mainWindow.isMinimized()) mainWindow.restore()
    mainWindow.focus()
  })
}

/**
 * 隐藏窗口菜单，但保留编辑类 role。
 *
 * Windows 上 Ctrl+C / Ctrl+V / Ctrl+A 等快捷键由菜单项提供；
 * 完全不设菜单会导致输入框里无法复制粘贴。
 */
function installMenu(): void {
  const template: Electron.MenuItemConstructorOptions[] = [
    {
      label: '编辑',
      submenu: [
        { role: 'undo', label: '撤销' },
        { role: 'redo', label: '重做' },
        { type: 'separator' },
        { role: 'cut', label: '剪切' },
        { role: 'copy', label: '复制' },
        { role: 'paste', label: '粘贴' },
        { role: 'selectAll', label: '全选' }
      ]
    },
    {
      label: '视图',
      submenu: [
        { role: 'reload', label: '重新加载' },
        // 开发者工具只在开发构建里出现
        ...(app.isPackaged
          ? []
          : ([{ role: 'toggleDevTools', label: '开发者工具' }] as Electron.MenuItemConstructorOptions[])),
        { type: 'separator' },
        { role: 'resetZoom', label: '实际大小' },
        { role: 'zoomIn', label: '放大' },
        { role: 'zoomOut', label: '缩小' }
      ]
    }
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

app.whenReady().then(() => {
  try {
    initDb()
  } catch (e) {
    // 数据库无法初始化时仍要给出可见反馈，而不是静默退出
    dialog.showErrorBox(
      '数据库初始化失败',
      `${e instanceof Error ? e.message : String(e)}

请删除用户数据目录下的 workbench.db 后重试。`
    )
  }
  installMenu()
  registerIpc()
  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

app.on('before-quit', () => {
  closeDb()
})
