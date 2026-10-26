---
name: release
description: 发布 Claude 工作台新版本：升版本号、打 tag、推送并触发 GitHub Actions 三平台构建，最后验证产物真的上线。当用户说「发布」「发版」「release」「出个新版本」「打个包发出去」「升个版本」「发一下」，或完成一批改动后想让自动更新能检测到时使用。即使用户只说「发一下」这类极简指令也应使用，不要凭记忆手敲发布命令。
allowed-tools: Bash(git:*), Bash(npm:*), Bash(node:*), Read, Edit
license: MIT
---

# 发布新版本

这个项目的发布是**推 tag 触发**，不是推代码触发。普通 `git push` 不会构建，
推一个 `v*` tag 才会让 GitHub Actions 跑三平台构建。

发布不可逆性很高：tag 推上去后要撤销得删 Release、删 tag、可能还要清 CI 记录。
所以先把状态查清楚，再动手。

## 一、前置检查（必须全过再继续）

依次确认以下四项。任何一项不通过就停下来告诉用户，不要"跳过凑合发"。

```bash
git status --porcelain          # 必须为空，否则会漏提交改动
git branch --show-current       # 应为 main
git remote -v                   # 必须有 origin 指向 GitHub
git fetch origin && \
  git rev-list --count origin/main..HEAD && \
  git rev-list --count HEAD..origin/main
```

最后两个数字都应为 `0`：本地没有未推的提交，远程也没有未拉取的提交。
如果有差异，先让用户决定合并还是推送，不要在分支不同步的状态下打 tag——
那样 Release 会指向一个 main 上不存在的 commit。

版本号与 tag 的一致性也很关键：`package.json` 的 `version` 决定 tag 名，
也决定客户端看到的版本。两者不一致会导致更新器判断错误。

## 二、决定版本号

按改动性质选，默认 `patch`：

| 命令 | 什么时候用 |
|---|---|
| `patch` | 修 bug、调样式、改文案 → 0.1.1 → 0.1.2 |
| `minor` | 加了用户能感知的功能 → 0.1.1 → 0.2.0 |
| `major` | 破坏性变更（数据结构、交互方式大改）→ 0.1.1 → 1.0.0 |

从对话上下文判断。用户明确说了「加了个功能」就用 `minor`，说了「修了个 bug」
就用 `patch`。判断不出来时用 `patch` 并说明理由，不要为了这点小事反复追问。

## 三、本地验证（不能省）

CI 也会跑这两步，但本地跑只要 2 秒。让 CI 替你发现类型错误意味着
你要等 4 分钟、拿到一个失败的 tag、再走一遍恢复流程。不值得。

```bash
npm run typecheck   # vue-tsc + tsc，零输出才算过
npm test            # 全绿，看 "Tests N passed"
```

有失败就先修，修完重跑，通过了再进入下一步。

## 四、升版本并提交

```bash
npm version patch --no-git-tag-version
```

`--no-git-tag-version` 是必要的：不加的话 npm 会自己 commit + 打 tag，
一旦工作区有临时改动就整体失败，而且 tag 的 message 由 npm 决定，不好控制。

这个命令会同时改 `package.json` 和 `package-lock.json`，两个文件都要提交。
漏掉 lock 会让 CI 的 `npm ci` 因为版本不一致而失败。

```bash
git add package.json package-lock.json
git commit -m "chore: 发布 0.1.2"    # 版本号替换成实际值
```

commit message 末尾加上仓库要求的署名尾注。

## 五、打 tag 并推送

先确认 tag 不存在，避免覆盖：

```bash
git rev-parse -q --verify refs/tags/v0.1.2 && echo "已存在，停止"
```

确认不存在后：

```bash
git tag -a v0.1.2 -m "v0.1.2"
git push origin main v0.1.2
```

`main` 和 tag 要在同一次推送里推上去。只推 tag 会让 Release 指向一个
远端 main 上还没有的 commit，用户点进 Release 看不到对应源码。

## 六、等待并验证 CI

用脚本轮询，不要手写 curl 加 JSON 解析：

```bash
node .claude/skills/release/scripts/wait-for-release.mjs v0.1.2
```

脚本会打印每个 job 的状态，成功退出码 0，失败退出码 1。
默认最长等 20 分钟——三平台构建实测约 4 分钟，留足余量。

如果环境里有 `GH_TOKEN`，脚本会自动带上以提高 API 限额；
没有也没关系，公开仓库允许匿名查询（每小时 60 次）。

## 七、确认产物真的上线

CI 绿了不代表用户能更新。还要确认 Release 里有客户端真正需要的文件：

```bash
node .claude/skills/release/scripts/wait-for-release.mjs v0.1.2 --verify
```

`--verify` 会检查这几项，缺任何一项都算发布失败：

- Windows：`ClaudeWorkbench-<版本>-x64.exe` + `.blockmap`
- Linux：`ClaudeWorkbench-<版本>-x86_64.AppImage`
- macOS：`.dmg` **和 `.zip`**（zip 是 Squirrel 自更新的必要产物）
- 三份元数据：`latest.yml`、`latest-mac.yml`、`latest-linux.yml`
- `latest.yml` 里的 `version` 与本次发布版本一致

最后一条最容易出问题：如果 `latest.yml` 的版本没变，客户端会永远认为
自己是最新版。脚本会直接把两个值打出来对照。

## 八、CI 失败怎么恢复

先看脚本输出定位是哪个平台挂了，再决定：

**代码问题**（typecheck/test 挂了 —— 但本地已经跑过，说明是环境差异）：
修代码，重新走一遍流程，但**先删掉坏 tag**：

```bash
git tag -d v0.1.2
git push origin :refs/tags/v0.1.2
```

再用同一个版本号重新打 tag 推送。不要留着失败的 release 给客户端看。

**单平台失败**（比如 macOS 签名相关）：
其他平台产物已经上传，可以保留。用
`gh workflow run release.yml` 或网页上的 Run workflow 重跑整条流水线。
注意重跑会在已存在的 Release 上覆盖同名产物。

**prepare job 失败**（创建 Release 那一步）：
通常是 `GITHUB_TOKEN` 权限问题。检查仓库 Settings → Actions →
General → Workflow permissions 是否允许读写。

## 不适合用这个 skill 的场景

- 只想验证代码没坏、不想发布 → 推普通提交，去网页手动触发 workflow
- 只想本地打包看效果 → `npm run dist`，不涉及 tag 和推送
- 改的是 CI 配置本身 → 先发一个测试版本验证，或者直接看 Actions 日志

## 发布完成后的汇报

用这张表交账，不要只说"发布成功了"：

| 项 | 内容 |
|---|---|
| 版本 | 0.1.1 → 0.1.2 |
| tag | v0.1.2 |
| CI | 三平台各自结论 |
| 产物 | 实际文件名与大小 |
| 更新元数据 | `latest.yml` 的 version 值 |
| Release 地址 | https://github.com/1696016910/CCWorkspace/releases/tag/v0.1.2 |
