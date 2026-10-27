# Tasks

## 1. 目录重构

- [x] 1.1 创建 `node/` 目录，用 `git mv` 移入 `src/`、`tests/`、`package.json`、`package-lock.json`、`electron.vite.config.ts`、`electron-builder.yml`、`tsconfig.json`、`tsconfig.node.json`、`tsconfig.web.json`、`vitest.config.ts`；根目录保留 `prototype/`、`openspec/`、`scripts/`、`README.md`、`20261023-项目方案.md`。验证：`git status` 显示全部为 rename，无删除+新增组合
- [x] 1.2 修正 `node/package.json` 中 `build`/`dist`/`test`/`typecheck` 等脚本的相对路径，修正 `node/vitest.config.ts` 与三个 tsconfig 的 `include`/`rootDir`。验证：在 `node/` 下执行 `npm run typecheck` 与 `npm test` 全绿（102 项）
- [x] 1.3 修正 `node/electron-builder.yml` 的 `directories.buildResources` 指向 `../build`，`files` 路径相应调整。验证：在 `node/` 下 `npm run build && npx electron-builder --win --dir` 成功产出 `node/release/win-unpacked/Claude 工作台.exe`，启动后 15 个 API 命名空间齐全、无 console 错误
- [x] 1.4 修正 `scripts/check-build-lock.mjs` 的根目录推导（当前从脚本位置推两层，迁移后需适配）与 `node/` 下的调用方式。验证：`node scripts/check-build-lock.mjs --path node/release` 能正确报告占用状态
- [x] 1.5 更新根 `README.md`，说明三个实现的目录结构与各自的构建命令。验证：按 README 写的命令能原样执行成功

## 2. CI 拆分

- [x] 2.1 把 `.github/workflows/release.yml` 拆为 `release-node.yml`（只构建 Node 版），修正其中所有路径为 `node/` 前缀、`working-directory` 为 `node`。验证：本地 `npx js-yaml` 解析通过；`workflow_dispatch` 手动触发后 Node 产物上传成功
- [x] 2.2 新建 `release-go.yml` 与 `release-rust.yml` 骨架，先只做 `workflow_dispatch` 手动触发，各自 `setup-go` / `setup-rust` 并跑构建命令。验证：两份文件 `js-yaml` 解析通过；手动触发能跑到构建步骤（此时产物可为空）
- [x] 2.3 三份 workflow 统一产物命名前缀 `ClaudeWorkbench-Node|Go|Rust-<version>-<arch>.<ext>`，避免同名文件在同一 Release 内冲突。验证：三个 workflow 的 `artifactName` 互不相同，`grep` 确认无重复

## 3. 协议文档

- [x] 3.1 新增 `docs/claude-cli-protocol.md`，记录 design.md 决策 2 中的启动参数、stdin 格式、stdout 事件表、必过滤项、错误分类。验证：文档中的命令可原样复制执行并得到逐行 JSON
- [x] 3.2 在文档中补充实测样本：`system/init`、`stream_event`（text_delta）、`assistant`（tool_use）、`result` 各一条真实片段（脱敏）。验证：每条样本都能用 `jq` 解析成功
- [x] 3.3 文档补充 hook 事件污染的实际案例（SessionStart 吐出数万字符）。验证：文档说明了按 `type`/`subtype` 过滤的具体条件

## 4. Go 版：基础设施

- [x] 4.1 初始化 `go/go.mod`（module `claude-workbench`），加入 `fyne.io/fyne/v2` 与 `modernc.org/sqlite`。验证：`go mod tidy` 成功，`go build ./...` 通过
- [x] 4.2 实现 `go/internal/db`：打开 SQLite、`PRAGMA integrity_check`、`foreign_keys=ON`、`busy_timeout=5000`、WAL；损坏时备份并重建；`schema_version < 4` 时报错提示先用 Node 版迁移。验证：`go test ./internal/db` 覆盖空库/损坏库/版本过旧三种情况
- [x] 4.3 实现 `go/internal/db` 的 workspaces / sessions / messages 三张表的读写与迁移校验。验证：单元测试断言表结构与 Node 版一致（列名、类型、索引名）

## 5. Go 版：claude 进程

- [x] 5.1 实现 `go/internal/claude`：按协议文档构造参数、`exec.CommandContext` 启动、stdin 写入 NDJSON、stdout 逐行扫描。验证：单元测试用假命令（`echo` 脚本）验证参数拼接与行解析
- [x] 5.2 实现事件解析：按 `type`/`subtype` 分派，过滤 `hook_*` 与非白名单类型，过滤 `parent_tool_use_id` 非空的子代理事件。验证：单元测试喂入含 hook 事件与子代理事件的样本流，断言只产出预期事件
- [x] 5.3 实现 30 秒空闲超时、中断（kill 进程）、错误分类（`is_error`、超时、无 result）。验证：单元测试用 sleep 脚本验证超时；用返回 `is_error` 的假流验证错误翻译

## 6. Go 版：业务与 UI

- [x] 6.1 实现工作区与会话服务（增删改查、软删除、恢复、彻底删除、复制、置顶）。验证：`go test ./internal/services` 覆盖 CRUD 与软删除语义
- [x] 6.2 实现配置读写（`~/.claude/settings.json` 的 env / permissions / models，写前备份、mtime 冲突检测）。验证：单元测试覆盖畸形 JSON 拒写、备份生成、env 键名校验
- [x] 6.3 实现 Skill 与插件列举（扫 `~/.claude/skills`、调 `claude plugin list --json`）。验证：单元测试用临时目录造 skill 文件，断言解析结果
- [x] 6.4 搭建 Fyne 主窗口：侧栏（工作区树 + 会话列表）、对话区、输入框、底部按钮。加载系统中文字体（msyh → deng → simhei 回退链）。验证：启动后界面无方框乱码，侧栏可折叠，会话可切换
- [x] 6.5 实现对话流式渲染：逐 token 追加、工具调用折叠、停止按钮、已停止提示、上下文百分比。验证：真实发一条消息，观察逐字输出；点停止后出现「已停止生成」
- [x] 6.6 实现设置页六个分区（工作区/环境变量/权限/模型/Skill/插件）。验证：每个分区能读写对应配置，重启后保持
- [ ] 6.7 实现新建工作区与删除确认弹窗。验证：新建后出现在侧栏；删除会话/工作区前弹确认，确认后执行

## 7. Rust 版：基础设施

- [ ] 7.1 初始化 `rust/Cargo.toml`，加入 `eframe`、`egui`、`rusqlite`（bundled）、`serde`、`serde_json`。验证：`cargo build` 通过
- [ ] 7.2 实现 `rust/src/db`：打开 SQLite、完整性检查、foreign_keys、busy_timeout、WAL、损坏重建、版本门禁。验证：`cargo test` 覆盖空库/损坏/版本过旧
- [ ] 7.3 实现三张表的读写，表结构与 Node 版一致。验证：测试断言 PRAGMA table_info 输出与 Node 版列名一致

## 8. Rust 版：claude 进程

- [ ] 8.1 实现 `rust/src/claude`：参数构造、`Command` 启动、stdin 写 NDJSON、`BufReader::lines()` 读。验证：测试用假命令验证参数与解析
- [ ] 8.2 实现事件解析与过滤（同 5.2 规则）。验证：测试喂入 hook 与子代理样本，断言过滤正确
- [ ] 8.3 实现超时、中断、错误分类。验证：测试覆盖三类错误路径

## 9. Rust 版：业务与 UI

- [ ] 9.1 实现工作区与会话服务。验证：`cargo test` 覆盖 CRUD 与软删除
- [ ] 9.2 实现配置读写。验证：测试覆盖畸形 JSON、备份、键名校验
- [ ] 9.3 实现 Skill 与插件列举。验证：测试用临时目录断言解析
- [ ] 9.4 搭建 egui 主窗口：侧栏、对话区、输入框、底部按钮，运行时加载系统中文字体。验证：界面无乱码，交互可用
- [ ] 9.5 实现对话流式渲染与停止。验证：真实对话逐字输出；停止后显示「已停止生成」
- [ ] 9.6 实现设置页六个分区。验证：每个分区读写正常，重启保持
- [ ] 9.7 实现弹窗（新建工作区、删除确认）。验证：交互流程完整

## 10. 集成验证

- [ ] 10.1 三份实现分别启动，各自创建的工作区/会话能被另外两份读到（同一 workbench.db）。验证：Go 版建会话 → 关掉 → Rust 版打开能看到 → Node 版也能看到
- [ ] 10.2 三份实现分别做一次完整对话（发消息 → 流式 → 工具调用 → 结束），对比事件序列一致。验证：三份各自的日志中 `start/text/done` 事件类型序列相同
- [ ] 10.3 UI 验证：三份实现逐一截图，对照 `prototype/20261027-界面原型.html` 检查信息结构（区域划分、控件存在性）。验证：每份至少截 4 张（主界面/对话/设置/弹窗）
- [ ] 10.4 打包验证：三份各出一个 Windows 安装包，记录体积。验证：三个 exe 都能安装并启动，Go/Rust 版 < 30MB
- [ ] 10.5 数据安全验证：`workbench.db` 在 Go/Rust 版操作后，Node 版仍能正常读取（无 schema 破坏）。验证：三份轮转启动，无报错
