import { call } from './invoke'
import type { SkillInfo } from './types'

export function list(projectDir?: string): Promise<SkillInfo[]> {
  return call('skill_list', { projectDir: projectDir ?? null })
}

export function toggle(name: string, enabled: boolean): Promise<SkillInfo[]> {
  return call('skill_toggle', { name, enabled })
}
