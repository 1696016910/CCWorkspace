import { app, BrowserWindow, dialog, ipcMain, Menu, screen, shell } from 'electron'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { readFileSync, writeFileSync } from 'node:fs'
import { initDb, closeDb } from './db'
import {
  addWorkspace,
  listWorkspaces,
  removeWorkspace,
  renameWorkspace,
  setWorkspacePinned,
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
  purgeSession,
  setSessionPinned
} from './services/session'
import {
  assertCanSend,
  compact,
  forgetContext,
  getContextUsage,
  interrupt,
  isRunning,
  listCommands,
  refreshContext,
  send
} from './services/chat'
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
import {
  checkForUpdate,
  checkOnStartup,
  downloadUpdate,
  getFeedPrefix,
  getUpdateStatus,
  installUpdate,
  openReleases,
  setFeedPrefix
} from './services/update'
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
      // 沙箱模式下 preload 只支持 CommonJS，而 electron-vite 输出的是 ESM(.mjs)，
      // 开启会导致 window.api 整个不存在、界面全白。
      // 真正的防护靠上面几层：导航精确比对、IPC 来源校验、CSP、DOMPurify。
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
  // 窗口销毁后必须置空：macOS 上关窗不退出进程，
  // 再次从 Dock 启动时 second-instance 会拿到已销毁对象并抛
  // "Object has been destroyed"，窗口再也打不开。
  mainWindow.on('closed', () => {
    mainWindow = null
  })

  mainWindow.on('resize', scheduleSave)
  mainWindow.on('move', scheduleSave)
  mainWindow.on('close', saveWindowState)

  /**
   * 只允许应用自身的页面加载。
   *
   * preload 会随每次导航重新注入，一旦窗口被导航到外部网址，
   * 那个网页的 JS 就能直接拿到 window.api 的 57 个通道，
   * 等于把读文件、改配置、装插件的能力交给任意网站。
   * 所以这里是安全边界，不是体验优化。
   */
  mainWindow.webContents.on('will-navigate', (event, url) => {
    if (isAppUrl(url)) return
    event.preventDefault()
    void openExternalSafely(url)
  })

  // 新开窗口一律拒绝，交给系统浏览器
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    void openExternalSafely(url)
    return { action: 'deny' }
  })

  if (process.env['ELECTRON_RENDERER_URL']) {
    void mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    void mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

/**
 * 只允许 http/https 交给系统浏览器。
 *
 * 自定义协议（ms-settings:、file: 等）能直接拉起本地程序或系统面板，
 * 这些入口全部来自不可信内容（模型输出、文件预览里的链接），必须挡住。
 */
async function openExternalSafely(url: string): Promise<void> {
  try {
    const parsed = new URL(url)
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return
    await shell.openExternal(url)
  } catch {
    // 畸形 URL 直接忽略，不弹错打扰用户
  }
}

/**
 * 校验 IPC 调用确实来自应用自己的页面。
 *
 * will-navigate 已经挡掉了大部分导航，但校验来源是最后一道防线：
 * 万一窗口被以别的方式加载了外部内容，这里能拦住它拿到文件读取、
 * 配置修改、插件安装这些能力。senderFrame 是 Electron 提供的真实来源，
 * 不能靠渲染层自己声明的参数。
 */
/**
 * 应用自身的页面地址。
 *
 * 不能用 `startsWith('file://')` 判断"是不是自己人"：
 * 任何本地 HTML 文件、UNC 共享（\\attacker\share\x.html）都是 file://，
 * 它们一旦被加载，preload 会重新注入，那个页面的脚本就能调用全部 IPC。
 */
function appUrl(): string {
  const devUrl = process.env['ELECTRON_RENDERER_URL']
  if (devUrl) return devUrl
  return pathToFileURL(join(__dirname, '../renderer/index.html')).href
}

/**
 * 规范化后精确比较，避免把同目录下的其它页面当成自己。
 *
 * 必须用 URL 解析而不是字符串比较：Chromium 会把 frame.url 规范化成
 * 带尾斜杠的形式（http://localhost:5173 → http://localhost:5173/），
 * 直接比字符串会让开发模式下所有 IPC 都被判成非法来源。
 */
function isAppUrl(url: string): boolean {
  try {
    const a = new URL(url)
    const b = new URL(appUrl())
    return a.origin === b.origin && a.pathname === b.pathname
  } catch {
    return false
  }
}

function fromOurPage(event: Electron.IpcMainInvokeEvent): boolean {
  const frame = event.senderFrame
  if (!frame) return false
  return isAppUrl(frame.url)
}

/** 统一把异常转成渲染层可读的错误，避免 IPC 调用直接 reject 而丢信息 */
function handle(channel: string, fn: (...args: never[]) => unknown): void {
  ipcMain.handle(channel, async (event, ...args) => {
    if (!fromOurPage(event)) throw new Error('非法调用来源')
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

  /**
   * 用系统浏览器打开外部链接。
   * 渲染层拦截了所有 <a> 点击，避免 Electron 窗口被导航走导致应用界面丢失。
   * 只接受 http/https，防止被诱导打开本地程序或自定义协议。
   */
  handle('system:openExternal', async (url: string) => {
    let parsed: URL
    try {
      parsed = new URL(url)
    } catch {
      throw new Error('无效的链接')
    }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      throw new Error('只支持 http/https 链接')
    }
    await shell.openExternal(url)
  })


  /**
   * 读取文件内容供应用内预览。
   * 只读不写，并对体积与二进制做限制，避免把大文件读进渲染进程。
   */
  handle('file:read', async (path: string) => {
    const { readFile, stat } = await import('node:fs/promises')
    const { extname, basename, resolve, sep } = await import('node:path')

    if (typeof path !== 'string' || !path.trim()) throw new Error('路径不能为空')
    // UNC（\\host\share）会在打开时向远端发起 SMB 认证，可能外泄 NTLM 凭据
    if (/^[\\/]{2}/.test(path)) throw new Error('不支持网络路径')

    /**
     * 只允许读取工作区内的文件。
     * 预览入口来自模型输出，属于不可信内容：不加限制的话，
     * 一个指向 ~/.ssh/id_rsa 或 .claude/settings.json 的芯片
     * 点一下就能把私钥 / API Token 明文显示在界面上。
     */
    const within = (p: string, root: string): boolean => {
      const base = root.endsWith(sep) ? root : root + sep
      return p === root || p.startsWith(base)
    }

    const { realpath } = await import('node:fs/promises')
    const literal = resolve(path)
    const roots = listWorkspaces().map((w) => resolve(w.path))
    if (!roots.some((r) => within(literal, r))) {
      throw new Error('只能预览当前工作区内的文件')
    }

    // 再解析一次真实路径：工作区内可以放一个指向 ~/.claude 的 junction/symlink，
    // 字面路径检查会放行，实际读到的却是工作区外的文件
    let target = literal
    try {
      const real = await realpath(literal)
      const realRoots = await Promise.all(
        roots.map((r) => realpath(r).catch(() => r))
      )
      if (!realRoots.some((r) => within(real, r))) {
        throw new Error('该路径指向工作区外，已阻止预览')
      }
      target = real
    } catch (e) {
      if (e instanceof Error && e.message.includes('工作区外')) throw e
      // 文件不存在等：后面统一报"无法访问"
    }

    let info
    try {
      info = await stat(target)
    } catch {
      throw new Error('文件不存在或无法访问')
    }
    if (!info.isFile()) throw new Error('该路径不是文件')

    const MAX = 2 * 1024 * 1024
    if (info.size > MAX) {
      throw new Error(`文件过大（${Math.round(info.size / 1024)} KB），暂不支持预览`)
    }

    const ext = extname(target).toLowerCase()
    const binaryExts = new Set([
      '.png', '.jpg', '.jpeg', '.gif', '.webp', '.ico', '.bmp',
      '.pdf', '.zip', '.gz', '.7z', '.rar', '.exe', '.dll', '.so',
      '.woff', '.woff2', '.ttf', '.mp3', '.mp4', '.mov',
      '.dylib', '.node', '.dmg', '.app', '.pkg', '.deb', '.AppImage'
    ])
    if (binaryExts.has(ext)) {
      throw new Error('这是二进制文件，无法以文本方式预览')
    }

    // 先按字节读入，再判断编码：Windows 上的中文文档常是 GBK
    let buf: Buffer
    try {
      buf = await readFile(target)
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
      path: target,
      name: basename(target),
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
  handle('workspace:add', (dir: string, name?: string) => addWorkspace(dir, name))
  handle('workspace:remove', (id: string) => removeWorkspace(id))
  handle('workspace:rename', (id: string, name: string) => renameWorkspace(id, name))
  handle('workspace:pin', (id: string, pinned: boolean) => setWorkspacePinned(id, pinned))
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
  handle('session:pin', (id: string, pinned: boolean) => setSessionPinned(id, pinned))
  handle('session:purge', (id: string) => {
    purgeSession(id)
    forgetContext(id)
  })

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

  /* Skill：只列全局 Skill，不含项目级 */
  handle('skill:list', () => listSkills(null))
  handle('skill:toggle', async (name: string, enabled: boolean) => {
    await toggleSkill(name, enabled)
    return listSkills(null)
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

  /* 更新 */
  handle('app:version', () => app.getVersion())
  handle('update:status', () => getUpdateStatus())
  handle('update:check', () => checkForUpdate())
  handle('update:download', () => downloadUpdate())
  handle('update:install', () => installUpdate())
  handle('update:getFeed', () => getFeedPrefix())
  handle('update:setFeed', (prefix: string) => setFeedPrefix(prefix))
  handle('update:openReleases', () => openReleases())
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
    if (!mainWindow || mainWindow.isDestroyed()) {
      createWindow()
      return
    }
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
  const isMac = process.platform === 'darwin'
  const template: Electron.MenuItemConstructorOptions[] = [
    // macOS 必须有应用菜单：第一个子菜单的 label 会被系统替换成应用名，
    // 缺了它就没有 Cmd+Q / About / Hide，用户关窗后会觉得"退不掉"
    ...(isMac ? ([{ role: 'appMenu' }] as Electron.MenuItemConstructorOptions[]) : []),
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
    },
    // 窗口菜单提供最小化/关闭/前置全部窗口，macOS 上属于标准菜单项
    ...(isMac ? ([{ role: 'windowMenu' }] as Electron.MenuItemConstructorOptions[]) : [])
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

app.whenReady().then(() => {
  // app.quit() 是异步的，拿不到锁时 whenReady 仍会触发。
  // 不在这里二次拦截，第二个实例会打开同一个数据库并并发迁移。
  if (!gotLock) return

  try {
    initDb()
  } catch (e) {
    // 数据库无法初始化时仍要给出可见反馈，而不是静默退出
    const detail = e instanceof Error ? e.message : String(e)
    dialog.showErrorBox(
      '数据库初始化失败',
      `${detail}

如果是提示"正被另一个进程使用"，请先关闭其他实例。
否则请备份用户数据目录下的 workbench.db 后再手动处理。`
    )
    app.quit()
    return
  }
  installMenu()
  registerIpc()
  createWindow()
  checkOnStartup()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

let quittingWhileBusy = false

/**
 * 退出前先收尾正在跑的回复。
 *
 * closeDb() 之后 appendMessage 会抛"数据库尚未初始化"，这一轮的回复
 * 就永久丢失了（用户白花了 token）。所以有请求在跑时先中断它，
 * 让 send() 走完自己的收尾路径再关库。
 */
app.on('before-quit', (event) => {
  if (isRunning() && !quittingWhileBusy) {
    event.preventDefault()
    quittingWhileBusy = true
    void interrupt()
    // 给收尾留出时间：写入消息、emit done、关闭迭代器
    setTimeout(() => {
      closeDb()
      app.quit()
    }, 800)
    return
  }
  closeDb()
})

/** 未捕获异常至少留下痕迹，否则问题会静默消失 */
process.on('uncaughtException', (e) => {
  console.error('[main] uncaughtException:', e)
})

process.on('unhandledRejection', (e) => {
  console.error('[main] unhandledRejection:', e)
})
