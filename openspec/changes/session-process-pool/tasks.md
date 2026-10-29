# Tasks

## 1. 进程池基础设施

- [x] 1.1 新建 `src-tauri/src/session.rs`：封装 `SessionProcess`，支持 spawn / run_turn / alive，
      读到 `result` 即本轮结束。验证：`cargo build` 通过
- [x] 1.2 新建 `src-tauri/src/pool.rs`：LRU 常驻池（容量 3）+ 空闲回收 + evict/shutdown。
      验证：5 个单元测试覆盖容量上限、空闲回收、取出放回、同会话复用、多会话独立
- [x] 1.3 `claude.rs` 开放 `build_args` / `build_command` / `kill_tree` / `is_valid_event` / `State`，
      供 session 模块复用。验证：`cargo build` 无警告

## 2. 接入对话链路

- [x] 2.1 `chat_send` 改为从池取进程；取不到就 spawn；正常结束放回池，中断/超时丢弃。
      验证：编译通过，`cargo test` 57 项通过
- [x] 2.2 `chat_interrupt` 中断时 `pool.evict`，保证下轮重建。验证：编译通过
- [x] 2.3 `session_remove` 删除会话时清理其进程。验证：编译通过
- [x] 2.4 应用退出时 `pool.shutdown()` 回收全部进程，避免僵尸。验证：编译通过

## 3. 启动与回收

- [x] 3.1 setup 里启动 sweeper 线程，每 60 秒扫一次空闲进程。
      验证：编译通过，日志里能看到「回收 N 个空闲会话进程」
- [x] 3.2 新增 `pool_status` 命令，暴露当前常驻数。验证：编译通过

## 4. 删掉自动上下文探测

- [x] 4.1 删除 `done` 之后的 `probe_context` 调用。
      验证：代码中不再有该调用点
- [x] 4.2 新增 `context_probe` 命令，改为用户打开面板时调用。验证：命令注册且编译通过
- [x] 4.3 `context_compact` 改用进程池，压缩与随后的 `/context` 探测复用同一个进程。
      验证：编译通过，压缩后进程被放回池

## 5. 前端按需查询

- [x] 5.1 `api/chat.ts` 加 `probeContext`。验证：类型检查通过
- [x] 5.2 `ChatPane.openContext` 触发探测，带 `probing` 状态。
      验证：类型检查通过
- [x] 5.3 `ContextPopover` 加 `probing` prop 与转圈 loading 态。
      验证：`npm run build` 通过

## 6. 流式推送改事件驱动

- [x] 6.1 `flusher` 从 `sleep(80ms)` 轮询改为 channel 阻塞等待。
      验证：编译通过，删除 `FLUSH_INTERVAL_MS`

## 7. 测试

- [x] 7.1 后端全量测试：`cargo test` 57 项通过，0 警告
- [x] 7.2 前端全量测试：`npm test` 31 项通过
- [x] 7.3 前端构建：`npm run build` 通过
- [ ] 7.4 实测：连续 5 轮对话，第 2 轮起单轮耗时应接近纯模型时间（约 4-5s），不再有 8.5s
- [ ] 7.5 实测：中断后再发消息，模型能引用中断前的内容（上下文接得上）
- [ ] 7.6 实测：打开 4 个会话，`pool_status` 显示常驻不超过 3
- [ ] 7.7 实测：空闲 5 分钟后进程被回收（看日志）
