# Spec Delta

## Purpose

让用户查看并编辑 Claude 的配置：权限规则、环境变量、模型与 Skill 视图，
同时保证界面在亮色与暗色主题下都清晰可读。

## ADDED Requirements

### Requirement: 权限规则以表格编辑

系统 SHALL 以表格形式展示允许规则与拒绝规则，两者各自独立成表。

每条规则 SHALL 独占一行，可逐条编辑与删除。

系统 SHALL 提供逐条添加规则的操作；一次操作只添加一行。

规则内容为空的行 SHALL 以视觉方式标示，并阻止保存。

同一表格内出现重复规则时，系统 SHALL 阻止保存并提示重复位置。

#### Scenario: 添加一条规则

- **WHEN** 用户在允许规则表下方点击添加
- **THEN** 表格出现一个新的空行

#### Scenario: 删除一条规则

- **WHEN** 用户点击某行的删除按钮
- **THEN** 该行从表格中移除

#### Scenario: 空行阻止保存

- **WHEN** 表格中存在内容为空的行
- **THEN** 该行以警示样式标示
- **THEN** 保存按钮不可用

#### Scenario: 重复规则阻止保存

- **WHEN** 同一表格内存在两条内容相同的规则
- **THEN** 系统提示重复，保存不执行

#### Scenario: 保存生效

- **WHEN** 规则表内容合法且用户点击保存
- **THEN** 规则被写入配置文件，并生成备份

### Requirement: 规则清单与编辑

系统 SHALL 提供规则（Rule）分区，列出用户级与项目级规则文件。

规则分区 SHALL 支持查看规则全文、新增、编辑、删除。

删除规则前 SHALL 要求二次确认。

#### Scenario: 列出规则

- **WHEN** 用户打开设置中的规则分区
- **THEN** 系统列出可用的规则文件，并标明来源是用户级还是项目级

#### Scenario: 查看规则全文

- **WHEN** 用户点击某个规则
- **THEN** 系统展示该规则的完整内容

#### Scenario: 新增规则

- **WHEN** 用户新建一条规则并保存
- **THEN** 新规则出现在列表中，并且文件确实落盘

#### Scenario: 删除规则

- **WHEN** 用户删除某条规则并确认
- **THEN** 该规则从列表与磁盘上消失

### Requirement: Skill 只读列表

Skill 分区 SHALL 列出可用的 Skill 及其描述与来源，不再提供启用开关。

Skill 分区 SHALL 支持查看 Skill 全文、新增、编辑、删除。

删除带附属文件的 Skill 前，确认提示 SHALL 说明会一并删除该目录下的其他文件。

#### Scenario: 查看 Skill 列表

- **WHEN** 用户打开 Skill 分区
- **THEN** 列表显示每个 Skill 的名称、描述与来源，且没有启用开关

#### Scenario: 查看 Skill 全文

- **WHEN** 用户点击某个 Skill
- **THEN** 系统展示该 Skill 的完整内容

#### Scenario: 删除带附属文件的 Skill

- **WHEN** 用户删除一个目录内含附属文件的 Skill
- **THEN** 确认提示说明这些文件会一并删除

