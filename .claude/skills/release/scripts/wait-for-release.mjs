#!/usr/bin/env node
/**
 * 等待某个 tag 的 GitHub Actions 构建完成，并可选校验发布产物。
 *
 *   node wait-for-release.mjs v0.1.2
 *   node wait-for-release.mjs v0.1.2 --verify
 *
 * 成功退出码 0，失败或超时退出码 1。
 * 需要 REPO 常量与仓库一致，改仓库时记得同步。
 */
const OWNER = '1696016910'
const REPO = 'CCWorkspace'
const API = `https://api.github.com/repos/${OWNER}/${REPO}`
const MAX_WAIT_MS = 20 * 60 * 1000
const POLL_MS = 15 * 1000

/**
 * 客户端真正需要的产物，缺一项都算发布不完整。
 *
 * 这个项目是 Tauri + NSIS，只出 Windows 安装包：
 *   CCWorkbench_<版本>_x64-setup.exe
 * 注意分隔符是下划线、带 -setup 后缀 —— 与 Electron 版的
 * CCWorkbench-<版本>-x64.exe 不同，别照抄旧规则。
 */
const REQUIRED = [
  {
    label: 'Windows 安装包',
    test: (n, v) => n === `CCWorkbench_${v}_x64-setup.exe`
  }
]

const args = process.argv.slice(2)
const tag = args.find((a) => !a.startsWith('--'))
const verify = args.includes('--verify')

if (!tag) {
  console.error('用法: node wait-for-release.mjs <tag> [--verify]')
  process.exit(1)
}

const headers = { accept: 'application/vnd.github+json' }
if (process.env.GH_TOKEN) headers.authorization = `Bearer ${process.env.GH_TOKEN}`

async function api(path) {
  const res = await fetch(`${API}${path}`, { headers })
  if (!res.ok) throw new Error(`GitHub API ${res.status}: ${path}`)
  return res.json()
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

/**
 * tag 不存在时没必要空等 20 分钟——通常是打错版本号或忘了推送。
 * 已存在但暂时没有 run 才是正常情况（GitHub 建 run 有延迟）。
 */
async function assertTagPushed() {
  const res = await fetch(`${API}/git/refs/tags/${tag}`, { headers })
  if (res.status === 404) {
    throw new Error(`远端不存在 tag ${tag}：确认已执行 git push origin ${tag}`)
  }
  if (!res.ok) throw new Error(`查询 tag 失败: HTTP ${res.status}`)
}

/**
 * 找到该 tag 对应的 workflow run。
 * tag 刚推送时 GitHub 可能还没建 run，所以要容忍一段时间查不到。
 */
async function findRun() {
  const data = await api(`/actions/runs?per_page=30`)
  return (data.workflow_runs || []).find((r) => r.head_branch === tag) ?? null
}

async function waitForRun() {
  const start = Date.now()
  while (Date.now() - start < MAX_WAIT_MS) {
    const run = await findRun()
    if (run) return run
    process.stdout.write('.')
    await sleep(POLL_MS)
  }
  throw new Error(`等待 ${Math.round(MAX_WAIT_MS / 60000)} 分钟仍未找到 tag ${tag} 的构建记录`)
}

async function waitForJobs(runId) {
  const start = Date.now()
  let last = ''
  while (Date.now() - start < MAX_WAIT_MS) {
    const { jobs = [] } = await api(`/actions/runs/${runId}/jobs`)
    const line = jobs
      .map((j) => `${j.name.replace(/^build /, '')}=${j.conclusion ?? j.status}`)
      .join('  ')
    if (line !== last) {
      console.log(line)
      last = line
    }
    const running = jobs.some((j) => j.status === 'in_progress' || j.status === 'queued')
    if (!running && jobs.length > 0) return jobs
    await sleep(POLL_MS)
  }
  throw new Error('构建超时')
}

function fmtSize(bytes) {
  return `${(bytes / 1048576).toFixed(1)} MB`
}

async function verifyAssets() {
  const version = tag.replace(/^v/, '')
  const release = await api(`/releases/tags/${tag}`)
  const assets = release.assets || []

  console.log('\n产物检查:')
  let missing = 0
  for (const req of REQUIRED) {
    const hit = assets.find((a) => req.test(a.name, version))
    if (hit) {
      console.log(`  ✓ ${req.label}  ${hit.name}  ${fmtSize(hit.size)}`)
    } else {
      console.log(`  ✗ ${req.label}  缺失`)
      missing++
    }
  }

  // 版本号对不上时客户端会永远认为自己是最新版，这是最隐蔽的失败
  const yml = assets.find((a) => a.name === 'latest.yml')
  if (yml) {
    const res = await fetch(yml.url, {
      headers: { ...headers, accept: 'application/octet-stream' }
    })
    const text = await res.text()
    const got = /^version:\s*(\S+)/m.exec(text)?.[1]
    if (got === version) {
      console.log(`  ✓ latest.yml version = ${got}`)
    } else {
      console.log(`  ✗ latest.yml version = ${got ?? '(读不到)'}，期望 ${version}`)
      missing++
    }
  }

  if (missing > 0) {
    console.log(`\n结论: 有 ${missing} 项不合格，客户端可能无法更新`)
    return false
  }
  console.log('\n结论: 产物齐全，客户端可正常更新')
  return true
}

try {
  console.log(`查找 tag ${tag} 的构建记录...`)
  await assertTagPushed()
  const run = await waitForRun()
  console.log(`找到 run #${run.run_number}，开始跟踪...\n`)

  const jobs = await waitForJobs(run.id)
  const failed = jobs.filter((j) => j.conclusion !== 'success')

  if (failed.length > 0) {
    console.log(`\n构建失败 (${failed.length}/${jobs.length} 个 job):`)
    for (const j of failed) {
      console.log(`  ${j.name}: ${j.conclusion}`)
      console.log(`    ${j.html_url}`)
    }
    process.exit(1)
  }

  console.log(`\n全部 ${jobs.length} 个 job 成功`)

  if (verify) {
    const ok = await verifyAssets()
    process.exit(ok ? 0 : 1)
  }

  console.log(`\nRelease: https://github.com/${OWNER}/${REPO}/releases/tag/${tag}`)
  console.log('提示: 加 --verify 可校验产物完整性')
  process.exit(0)
} catch (e) {
  console.error(`\n错误: ${e instanceof Error ? e.message : String(e)}`)
  process.exit(1)
}
