# configuration Specification

## Purpose
让用户在不手工编辑 JSON 的前提下查看与修改 Claude 的环境变量、权限规则与模型映射，同时保证写坏配置不会导致 Claude 无法启动。

## Requirements

### Requirement: 读取并展示配置

系统 SHALL 读取 `~/.claude/settings.json`，并展示其中的 `env`、`permissions`、模型相关字段。

系统 MUST 在文件不存在时以空配置展示，而非报错退出。

#### Scenario: 展示环境变量

- **WHEN** 用户打开配置页的环境变量分区
- **THEN** 列出 `settings.json` 中 `env` 下的全部键值对

#### Scenario: 配置文件不存在

- **WHEN** `~/.claude/settings.json` 不存在
- **THEN** 界面显示空的环境变量列表
- **THEN** 用户可以直接新增第一条配置

### Requirement: 敏感值脱敏

系统 MUST 对键名匹配 `*_TOKEN`、`*_KEY`、`*_SECRET` 的取值默认脱敏显示。

用户 SHALL 能临时切换为明文查看。

#### Scenario: 令牌默认隐藏

- **WHEN** 用户打开环境变量分区，其中包含 `ANTHROPIC_AUTH_TOKEN`
- **THEN** 该值以掩码形式显示
- **THEN** 用户点击显示按钮后可见明文

### Requirement: 编辑环境变量

用户 SHALL 能新增、修改、删除环境变量条目。

系统 MUST 在保存前校验：键名非空、键名不重复、值不含换行符。

校验失败时 MUST 阻止保存并指出具体问题。

#### Scenario: 新增一条变量

- **WHEN** 用户新增键 `API_TIMEOUT_MS` 与值 `600000` 并保存
- **THEN** 该条目写入 `settings.json` 的 `env` 中

#### Scenario: 键名为空被拒绝

- **WHEN** 用户保存一条键名为空的条目
- **THEN** 保存被阻止，界面提示键名不能为空

#### Scenario: 重复键被拒绝

- **WHEN** 用户添加一个已存在的键名
- **THEN** 保存被阻止，界面提示键名重复

### Requirement: 写入前备份

系统 MUST 在修改 `settings.json` 前将原文件备份到 `~/.claude/backups/`，文件名包含时间戳。

系统 MUST 采用原子写入：先写临时文件再替换目标文件。

#### Scenario: 保存后存在备份

- **WHEN** 用户保存一次配置修改
- **THEN** `~/.claude/backups/` 下新增一个带时间戳的备份文件
- **THEN** 该备份内容为修改前的原文件

### Requirement: 并发修改检测

系统 MUST 在写入前检查目标文件的修改时间是否与读取时一致。

若不一致，系统 MUST 阻止写入并提示用户重新加载，MUST NOT 覆盖他人的修改。

#### Scenario: 文件在编辑期间被外部修改

- **WHEN** 用户打开配置页后，另一个进程修改了 `settings.json`，随后用户点击保存
- **THEN** 系统提示文件已被其它进程修改
- **THEN** 写入被阻止，用户的编辑内容不丢失

### Requirement: 变更生效范围提示

系统 MUST 在保存配置后提示：已在运行的会话不会读取新的环境变量。

#### Scenario: 保存后提示重启

- **WHEN** 用户保存环境变量修改
- **THEN** 界面提示需要重新打开会话才能生效

### Requirement: 权限与模型配置

系统 SHALL 允许用户查看与修改默认权限模式、允许规则、拒绝规则。

系统 SHALL 允许用户查看与修改模型相关配置。

当权限模式为 `bypassPermissions` 时，系统 MUST 显著提示其风险。

当拒绝规则为空时，系统 SHALL 提示建议添加基础保护规则。

#### Scenario: 高危权限模式提示

- **WHEN** 配置中的默认权限模式为 `bypassPermissions`
- **THEN** 权限分区显示醒目的风险提示

#### Scenario: 修改模型映射

- **WHEN** 用户修改主模型别名并保存
- **THEN** `settings.json` 中对应的模型字段更新为新值
- **THEN** 新建的会话使用新模型

### Requirement: 连接体检

用户 SHALL 能触发一次连通性检查，验证当前环境变量下的服务是否可用。

检查结果 SHALL 明确显示成功或失败。

#### Scenario: 连接正常

- **WHEN** 用户点击「连接体检」且当前配置可用
- **THEN** 界面显示检查成功及耗时

#### Scenario: 连接失败

- **WHEN** 用户点击「连接体检」且当前配置不可用
- **THEN** 界面显示失败原因
