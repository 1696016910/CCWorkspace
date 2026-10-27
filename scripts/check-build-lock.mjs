#!/usr/bin/env node
/**
 * 打包前的占用检查。
 *
 * electron-builder 会先 unlink 整个 win-unpacked 目录再重建。如果目录里的
 * 文件被别的程序持有句柄，就会报 EBUSY 并中断构建。
 *
 * 实测最常见的占用者不是本应用残留进程，而是编辑器：
 * VS Code 打开着这个项目时，它的文件监视器会持有 release/ 内文件的句柄。
 *
 * 用 Windows Restart Manager（rstrtmgr.dll）查谁锁着文件 —— 这是
 * "哪个进程打开了这个文件"的权威答案，比遍历进程模块可靠得多。
 *
 *   node scripts/check-build-lock.mjs            # 检查 release/
 *   node scripts/check-build-lock.mjs --kill     # 结束本项目的残留进程
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const target = join(root, 'release')
const kill = process.argv.includes('--kill')

if (!existsSync(target)) {
  console.log('release/ 不存在，无需检查')
  process.exit(0)
}

if (process.platform !== 'win32') {
  console.log('非 Windows 平台，跳过句柄检查')
  process.exit(0)
}

/**
 * 通过 Restart Manager 查询锁定某个路径的进程。
 * PowerShell 里内联 C# 是因为 Node 无法直接调 ntdll 的句柄表。
 */
const ps = `
$code = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class RmLock {
  [StructLayout(LayoutKind.Sequential)] struct RM_UNIQUE_PROCESS { public int dwProcessId; public System.Runtime.InteropServices.ComTypes.FILETIME ProcessStartTime; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct RM_PROCESS_INFO {
    public RM_UNIQUE_PROCESS Process;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strAppName;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=64)] public string strServiceShortName;
    public int ApplicationType; public uint AppStatus; public uint TSSessionId; [MarshalAs(UnmanagedType.Bool)] public bool bRestartable;
  }
  [DllImport("rstrtmgr.dll", CharSet=CharSet.Unicode)] static extern int RmRegisterResources(uint h, uint nFiles, string[] files, uint nApps, IntPtr apps, uint nSvcs, string[] svcs);
  [DllImport("rstrtmgr.dll")] static extern int RmStartSession(out uint h, int flags, string key);
  [DllImport("rstrtmgr.dll")] static extern int RmEndSession(uint h);
  [DllImport("rstrtmgr.dll")] static extern int RmGetList(uint h, out uint needed, ref uint count, IntPtr apps, ref uint reasons);
  public static List<string> Who(string path) {
    uint h; var res = new List<string>();
    if (RmStartSession(out h, 0, Guid.NewGuid().ToString()) != 0) return res;
    try {
      RmRegisterResources(h, 1, new[]{path}, 0, IntPtr.Zero, 0, null);
      uint needed = 0, count = 0, reasons = 0;
      RmGetList(h, out needed, ref count, IntPtr.Zero, ref reasons);
      if (needed == 0) return res;
      count = needed;
      int sz = Marshal.SizeOf(typeof(RM_PROCESS_INFO));
      IntPtr buf = Marshal.AllocHGlobal((int)count * sz);
      try {
        if (RmGetList(h, out needed, ref count, buf, ref reasons) == 0) {
          for (int i = 0; i < count; i++) {
            var info = (RM_PROCESS_INFO)Marshal.PtrToStructure(IntPtr.Add(buf, i * sz), typeof(RM_PROCESS_INFO));
            res.Add(info.Process.dwProcessId + "|" + info.strAppName);
          }
        }
      } finally { Marshal.FreeHGlobal(buf); }
    } finally { RmEndSession(h); }
    return res;
  }
}
'@
Add-Type -TypeDefinition $code -Language CSharp -ErrorAction Stop
# 只查一个代表性文件即可：整目录被同一批进程监视
$probe = Get-ChildItem -Path '${target.replace(/'/g, "''")}' -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $probe) { 'EMPTY' } else {
  $who = [RmLock]::Who($probe.FullName)
  if ($who.Count -eq 0) { 'NONE' } else { $who }
}
`

let out
try {
  out = execFileSync('powershell', ['-NoProfile', '-Command', ps], { encoding: 'utf-8' }).trim()
} catch (e) {
  console.log('句柄查询失败（不影响打包，可忽略）:', e.message.split('\n')[0])
  process.exit(0)
}

if (out === 'EMPTY' || out === 'NONE') {
  console.log('release/ 未被任何进程占用')
  process.exit(0)
}

const holders = out
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean)
  .map((l) => {
    const [pid, name] = l.split('|')
    return { pid: Number(pid), name }
  })

console.log(`release/ 正被 ${holders.length} 个进程占用：`)
for (const h of holders) console.log(`  ${h.name} (PID ${h.pid})`)

// 本项目自己的进程可以安全结束；编辑器等其他程序只提示不动手
const own = holders.filter((h) => /Claude 工作台|claude\.exe/i.test(h.name))
const others = holders.filter((h) => !own.includes(h))

if (own.length > 0 && kill) {
  for (const h of own) {
    try {
      execFileSync('powershell', ['-NoProfile', '-Command', `Stop-Process -Id ${h.pid} -Force`], {
        stdio: 'ignore'
      })
      console.log(`  已结束 ${h.name} (${h.pid})`)
    } catch {
      console.log(`  结束失败 ${h.name} (${h.pid})`)
    }
  }
}

if (others.length > 0) {
  console.log(
    '\n其中编辑器/其他程序不会被自动结束。' +
      '\n处理方式：关闭它们对该目录的监视，或构建到其他目录：' +
      '\n  npx electron-builder --win -c.directories.output=release-tmp'
  )
  process.exit(1)
}

process.exit(0)
