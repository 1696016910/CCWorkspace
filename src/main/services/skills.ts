import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import type { SkillInfo } from '../../shared/types'
import { claudeDir } from './paths'
import { getDb } from '../db'

/** 从 SKILL.md 的 frontmatter 里取 name / description，失败则回退到目录名 */
function parseFrontmatter(text: string): { name?: string; description?: string } {
  const match = /^---\r?\n([\s\S]*?)\r?\n---/.exec(text)
  if (!match) return {}
  const body = match[1] ?? ''
  const out: Record<string, string> = {}
  for (const line of body.split(/\r?\n/)) {
    const idx = line.indexOf(':')
    if (idx <= 0) continue
    const key = line.slice(0, idx).trim()
    const value = line.slice(idx + 1).trim().replace(/^["']|["']$/g, '')
    if (key && value) out[key] = value
  }
  return { name: out['name'], description: out['description'] }
}

/** 扫描一个目录下的 SKILL.md */
async function scanDir(dir: string, source: SkillInfo['source']): Promise<SkillInfo[]> {
  let entries
  try {
    entries = await readdir(dir, { withFileTypes: true })
  } catch {
    return []
  }

  const result: SkillInfo[] = []
  for (const entry of entries) {
    if (!entry.isDirectory()) continue
    const file = join(dir, entry.name, 'SKILL.md')
    try {
      const text = await readFile(file, 'utf-8')
      const fm = parseFrontmatter(text)
      result.push({
        name: fm.name || entry.name,
        description: fm.description || '',
        source,
        enabled: true
      })
    } catch {
      // 没有 SKILL.md 的目录不是 Skill
    }
  }
  return result
}

/** 读取被用户禁用的 Skill 名单 */
export function disabledSkills(): string[] {
  const row = getDb()
    .prepare("SELECT value FROM meta WHERE key = 'disabled_skills'")
    .get() as { value?: string } | undefined
  if (!row?.value) return []
  try {
    const parsed = JSON.parse(row.value)
    return Array.isArray(parsed) ? (parsed as string[]) : []
  } catch {
    return []
  }
}

function saveDisabled(names: string[]): void {
  getDb()
    .prepare(
      "INSERT INTO meta(key, value) VALUES('disabled_skills', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    )
    .run(JSON.stringify(names))
}

/** 列出用户级与项目级 Skill；项目目录取自当前工作区 */
export async function listSkills(projectDir: string | null): Promise<SkillInfo[]> {
  const userSkills = await scanDir(join(claudeDir(), 'skills'), 'user')
  const projectSkills = projectDir
    ? await scanDir(join(projectDir, '.claude', 'skills'), 'project')
    : []

  const disabled = new Set(disabledSkills())
  return [...userSkills, ...projectSkills].map((s) => ({
    ...s,
    enabled: !disabled.has(s.name)
  }))
}

export async function toggleSkill(name: string, enabled: boolean): Promise<void> {
  const current = new Set(disabledSkills())
  if (enabled) current.delete(name)
  else current.add(name)
  saveDisabled([...current])
}

/** 供会话启动时使用：返回允许加载的 Skill 名称列表 */
export function enabledSkillNames(all: SkillInfo[]): string[] {
  return all.filter((s) => s.enabled).map((s) => s.name)
}
