import { call } from './invoke'
import type { CommandInfo, RuleInfo, SkillInfo } from './types'

export function list(): Promise<SkillInfo[]> {
  return call('skill_list')
}

export function read(name: string, source: string, projectDir?: string | null): Promise<string> {
  return call('skill_read', { name, source, projectDir: projectDir ?? null })
}

export function write(
  name: string,
  source: string,
  content: string,
  projectDir?: string | null
): Promise<void> {
  return call('skill_write', { name, source, projectDir: projectDir ?? null, content })
}

export function remove(name: string, source: string, projectDir?: string | null): Promise<void> {
  return call('skill_delete', { name, source, projectDir: projectDir ?? null })
}

// ── 规则 ──

export function ruleList(projectDir?: string | null): Promise<RuleInfo[]> {
  return call('rule_list', { projectDir: projectDir ?? null })
}

export function ruleRead(name: string, source: string, projectDir?: string | null): Promise<string> {
  return call('rule_read', { name, source, projectDir: projectDir ?? null })
}

export function ruleWrite(
  name: string,
  source: string,
  content: string,
  projectDir?: string | null
): Promise<void> {
  return call('rule_write', { name, source, projectDir: projectDir ?? null, content })
}

export function ruleRemove(name: string, source: string, projectDir?: string | null): Promise<void> {
  return call('rule_delete', { name, source, projectDir: projectDir ?? null })
}

// ── 命令 ──

export function commandList(projectDir?: string | null): Promise<CommandInfo[]> {
  return call('command_list', { projectDir: projectDir ?? null })
}
