# Spec Delta

## Purpose

展示本机可用的 Skill 并允许用户控制哪些在新会话中加载，避免无关 Skill 占用上下文预算。

## ADDED Requirements

### Requirement: 发现并列出 Skill

系统 MUST 从用户级目录 `~/.claude/skills/` 与项目级目录 `<工作区>/.claude/skills/` 发现 Skill。

系统 MUST 从插件中发现的 Skill 一并列出，并标明其来源为插件。

每个 Skill 条目 SHALL 显示名称、来源（用户级 / 项目级 / 插件）、描述。

#### Scenario: 列出用户级与项目级 Skill

- **WHEN** 用户打开 Skill 管理界面
- **THEN** 同时显示用户级与当前项目级目录中的 Skill
- **THEN** 每个条目标注其来源

#### Scenario: 目录为空

- **WHEN** 用户级与项目级目录均不存在或为空
- **THEN** 界面显示空状态提示，而非空白页或错误

### Requirement: 启用与禁用 Skill

用户 SHALL 能切换单个 Skill 的启用状态。

禁用状态 MUST 影响新建立的会话；已在运行的会话不受影响。

系统 MUST 在切换后提示该变更对未来会话生效。

#### Scenario: 禁用一个 Skill

- **WHEN** 用户关闭某个 Skill 的开关
- **THEN** 该 Skill 标记为禁用
- **THEN** 新建的会话不再加载该 Skill

#### Scenario: 禁用不影响运行中的会话

- **WHEN** 用户禁用一个 Skill，同时已有一个会话正在运行
- **THEN** 正在运行的会话继续可用，其行为不立即改变

### Requirement: 变更持久化

系统 MUST 将 Skill 的启用状态持久化，使应用重启后保持用户的选择。

#### Scenario: 重启后状态保持

- **WHEN** 用户禁用某 Skill 后重启应用
- **THEN** 该 Skill 仍显示为禁用状态

### Requirement: Skill 只读展示

系统 MUST NOT 在应用内提供编辑 Skill 内容的能力。

用户 SHALL 能查看 Skill 的说明信息。

#### Scenario: 查看 Skill 说明

- **WHEN** 用户查看某个 Skill 条目
- **THEN** 显示其描述信息
- **THEN** 界面上没有编辑其内容的入口
