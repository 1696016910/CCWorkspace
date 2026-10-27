package ui

import (
	"claude-workbench/internal/model"
	"claude-workbench/internal/service"
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
	"fmt"
	"image/color"
)

// appVersion 由 app 包注入，用于「更新」分区展示。
var appVersion = "0.0.0"

// SetVersion 由 app 包在启动时调用。
func SetVersion(v string) { appVersion = v }

// settingsView 设置页：左侧导航 + 右侧内容。
type settingsView struct {
	win     fyne.Window
	content *fyne.Container
	nav     *widget.List
	active  int
	onClose func()
}

var settingTabs = []struct {
	label string
	build func(fyne.Window, func()) fyne.CanvasObject
}{
	{"工作区", buildWorkspaceSettings},
	{"环境变量", buildEnvSettings},
	{"权限", buildPermissionSettings},
	{"模型", buildModelSettings},
	{"Skill", buildSkillSettings},
	{"插件", buildPluginSettings},
	{"插件仓库", buildMarketplaceSettings},
	{"更新", buildUpdateSettings},
}

// ShowSettings 打开设置页。
func (m *MainWindow) ShowSettings() {
	sv := &settingsView{win: m.win, onClose: func() {}}

	sv.nav = widget.NewList(
		func() int { return len(settingTabs) },
		func() fyne.CanvasObject { return widget.NewLabel("") },
		func(i widget.ListItemID, o fyne.CanvasObject) {
			o.(*widget.Label).SetText(settingTabs[i].label)
		},
	)
	sv.content = container.NewMax()
	sv.nav.OnSelected = func(i widget.ListItemID) {
		sv.active = i
		sv.content.Objects = []fyne.CanvasObject{
			container.NewVScroll(settingTabs[i].build(sv.win, func() { sv.refresh() })),
		}
		sv.content.Refresh()
	}

	backBtn := widget.NewButtonWithIcon("返回", theme.NavigateBackIcon(), func() {
		m.win.SetContent(m.mainContent())
	})
	head := container.NewBorder(nil, nil, backBtn, nil, widget.NewLabelWithStyle("设置", fyne.TextAlignCenter, fyne.TextStyle{Bold: true}))

	body := container.NewHSplit(container.NewVScroll(sv.nav), container.NewPadded(sv.content))
	body.SetOffset(0.22)

	m.win.SetContent(container.NewBorder(head, nil, nil, nil, body))
	sv.nav.Select(0)
}

func (sv *settingsView) refresh() {
	if sv.active >= 0 && sv.active < len(settingTabs) {
		sv.content.Objects = []fyne.CanvasObject{
			container.NewVScroll(settingTabs[sv.active].build(sv.win, func() { sv.refresh() })),
		}
		sv.content.Refresh()
	}
}

// ── 分区构建 ──

func sectionTitle(text string) fyne.CanvasObject {
	l := widget.NewLabelWithStyle(text, fyne.TextAlignLeading, fyne.TextStyle{Bold: true})
	return l
}

// ── 工作区 ──

func buildWorkspaceSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("工作区"))
	box.Add(widget.NewLabel("工作区是一个项目目录。每个工作区下可以建多个会话。"))
	box.Add(widget.NewSeparator())

	list, _ := service.ListWorkspaces()
	if len(list) == 0 {
		box.Add(widget.NewLabel("还没有工作区"))
	}
	for _, w := range list {
		w := w
		pinLabel := "置顶"
		if w.PinnedAt != nil {
			pinLabel = "取消置顶"
		}
		row := container.NewBorder(nil, nil,
			nil,
			container.NewHBox(
				widget.NewButton(pinLabel, func() {
					service.PinWorkspace(w.ID, w.PinnedAt == nil)
					refresh()
				}),
				widget.NewButton("重命名", func() {
					entry := widget.NewEntry()
					entry.SetText(w.Name)
					dialog.ShowForm("重命名工作区", "保存", "取消",
						[]*widget.FormItem{widget.NewFormItem("名称", entry)},
						func(ok bool) {
							if !ok {
								return
							}
							if _, err := service.RenameWorkspace(w.ID, entry.Text); err != nil {
								dialog.ShowError(err, win)
								return
							}
							refresh()
						}, win)
				}),
				widget.NewButton("移除", func() {
					dialog.ShowConfirm("移除工作区",
						"移除「"+w.Name+"」会同时删除它下面的所有会话与消息，且无法恢复。\n磁盘上的项目目录不会被删除。",
						func(ok bool) {
							if !ok {
								return
							}
							if _, err := service.RemoveWorkspace(w.ID); err != nil {
								dialog.ShowError(err, win)
								return
							}
							refresh()
						}, win)
				}),
			),
			container.NewVBox(
				widget.NewLabelWithStyle(w.Name, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
				widget.NewLabel(w.Path),
			),
		)
		box.Add(row)
		box.Add(widget.NewSeparator())
	}

	pathE := widget.NewEntry()
	pathE.SetPlaceHolder("项目目录绝对路径")
	nameE := widget.NewEntry()
	nameE.SetPlaceHolder("名称（留空用目录名）")
	box.Add(widget.NewForm(
		widget.NewFormItem("目录", pathE),
		widget.NewFormItem("名称", nameE),
	))
	box.Add(widget.NewButton("添加工作区", func() {
		if _, err := service.AddWorkspace(pathE.Text, nameE.Text); err != nil {
			dialog.ShowError(err, win)
			return
		}
		pathE.SetText("")
		nameE.SetText("")
		refresh()
	}))
	return box
}

// ── 环境变量 ──

func buildEnvSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("环境变量"))
	box.Add(widget.NewLabel("写入 ~/.claude/settings.json 的 env 字段，保存前会自动备份。"))
	box.Add(widget.NewSeparator())

	entries, _ := service.ReadEnv()
	rows := container.NewVBox()
	edits := make([]*widget.Entry, 0, len(entries)*2)
	pairs := make([]struct{ k, v *widget.Entry }, 0, len(entries))

	for _, e := range entries {
		kE := widget.NewEntry()
		kE.SetText(e.Key)
		vE := widget.NewEntry()
		vE.SetText(e.Value)
		edits = append(edits, kE, vE)
		pairs = append(pairs, struct{ k, v *widget.Entry }{kE, vE})
		rows.Add(container.NewBorder(nil, nil, nil,
			widget.NewButton("删除", func() {
				// 删除由下面的"保存"统一生效：这里只是清空键名
				kE.SetText("")
			}),
			container.NewGridWithColumns(2, kE, vE),
		))
	}
	box.Add(rows)

	box.Add(widget.NewButton("添加一行", func() {
		kE := widget.NewEntry()
		vE := widget.NewEntry()
		edits = append(edits, kE, vE)
		pairs = append(pairs, struct{ k, v *widget.Entry }{kE, vE})
		rows.Add(container.NewGridWithColumns(2, kE, vE))
		rows.Refresh()
	}))

	box.Add(widget.NewLabel("这些值会以明文保存在本地配置文件中，不会上传。"))

	box.Add(widget.NewButton("保存", func() {
		var list []model.EnvEntry
		for _, p := range pairs {
			if p.k.Text == "" {
				continue // 空键名视为删除
			}
			list = append(list, model.EnvEntry{Key: p.k.Text, Value: p.v.Text})
		}
		if err := service.SaveEnv(list); err != nil {
			dialog.ShowError(err, win)
			return
		}
		refresh()
	}))
	_ = edits
	return box
}

// ── 权限 ──

func buildPermissionSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("权限"))
	box.Add(widget.NewLabel("控制 Claude 执行工具时的确认行为。"))
	box.Add(widget.NewSeparator())

	perm := service.ReadPermissions()
	modes := []string{"default", "acceptEdits", "plan", "dontAsk", "bypassPermissions"}
	sel := widget.NewSelect(modes, nil)
	sel.SetSelected(perm.DefaultMode)
	box.Add(widget.NewForm(widget.NewFormItem("默认模式", sel)))

	allowE := widget.NewMultiLineEntry()
	allowE.SetText(joinLines(perm.Allow))
	allowE.SetMinRowsVisible(4)
	box.Add(widget.NewLabel("允许规则（每行一条）"))
	box.Add(allowE)

	denyE := widget.NewMultiLineEntry()
	denyE.SetText(joinLines(perm.Deny))
	denyE.SetMinRowsVisible(4)
	box.Add(widget.NewLabel("拒绝规则（每行一条）"))
	box.Add(denyE)

	box.Add(widget.NewButton("保存", func() {
		err := service.SavePermissions(service.PermissionsModel{
			DefaultMode: sel.Selected,
			Allow:       splitLines(allowE.Text),
			Deny:        splitLines(denyE.Text),
		})
		if err != nil {
			dialog.ShowError(err, win)
			return
		}
		refresh()
	}))
	return box
}

// ── 模型 ──

func buildModelSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("模型"))
	box.Add(widget.NewLabel("模型只能在这里修改，会话内不提供切换。这些值存放在 settings.json 的 env 中。"))
	box.Add(widget.NewSeparator())

	fields := []struct{ key, label string }{
		{"ANTHROPIC_MODEL", "主模型"},
		{"ANTHROPIC_DEFAULT_OPUS_MODEL", "Opus 映射"},
		{"ANTHROPIC_DEFAULT_SONNET_MODEL", "Sonnet 映射"},
		{"ANTHROPIC_DEFAULT_HAIKU_MODEL", "Haiku 映射"},
		{"CLAUDE_CODE_EFFORT_LEVEL", "推理强度"},
		{"MAX_THINKING_TOKENS", "思考预算"},
	}
	models := service.ReadModels()
	entries := map[string]*widget.Entry{}
	for _, f := range fields {
		e := widget.NewEntry()
		e.SetText(models[f.key])
		entries[f.key] = e
		box.Add(widget.NewForm(widget.NewFormItem(f.label+"（"+f.key+"）", e)))
	}

	box.Add(widget.NewButton("保存", func() {
		next := map[string]string{}
		for k, e := range entries {
			next[k] = e.Text
		}
		if err := service.SaveModels(next); err != nil {
			dialog.ShowError(err, win)
			return
		}
		refresh()
	}))
	return box
}

// ── Skill ──

func buildSkillSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("Skill"))
	box.Add(widget.NewLabel("扫描 ~/.claude/skills。开关只影响新会话。"))
	box.Add(widget.NewSeparator())

	skills, _ := service.ListSkills()
	if len(skills) == 0 {
		box.Add(widget.NewLabel("未找到任何 Skill"))
	}
	for _, s := range skills {
		s := s
		chk := widget.NewCheck(s.Name, func(on bool) {
			if err := service.ToggleSkill(s.Name, on); err != nil {
				dialog.ShowError(err, win)
			}
		})
		chk.SetChecked(s.Enabled)
		box.Add(chk)
		if s.Description != "" {
			d := widget.NewLabel(s.Description)
			d.Wrapping = fyne.TextWrapWord
			box.Add(container.NewPadded(d))
		}
	}
	return box
}

// ── 插件 ──

func buildPluginSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("插件"))
	box.Add(widget.NewLabel("已安装的插件。启用或禁用会影响下一次会话。"))
	box.Add(widget.NewSeparator())

	plugins, _ := service.ListPlugins()
	if len(plugins) == 0 {
		box.Add(widget.NewLabel("没有已安装的插件，或 claude 命令不可用"))
	}
	for _, p := range plugins {
		p := p
		chk := widget.NewCheck(fmt.Sprintf("%s  %s", p.Name, p.Version), func(on bool) {
			if err := service.TogglePlugin(p.ID, on); err != nil {
				dialog.ShowError(err, win)
				return
			}
			refresh()
		})
		chk.SetChecked(p.Enabled)
		box.Add(chk)
	}
	return box
}

// ── 插件仓库 ──

func buildMarketplaceSettings(win fyne.Window, refresh func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("插件仓库"))
	box.Add(widget.NewLabel("添加第三方插件来源。"))
	box.Add(widget.NewSeparator())

	list, _ := service.ListMarketplaces()
	for _, m := range list {
		m := m
		var right fyne.CanvasObject
		if m.Builtin {
			right = widget.NewLabel("内置")
		} else {
			right = widget.NewButton("移除", func() {
				if err := service.RemoveMarketplace(m.Name); err != nil {
					dialog.ShowError(err, win)
					return
				}
				refresh()
			})
		}
		box.Add(container.NewBorder(nil, nil, nil, right,
			container.NewVBox(
				widget.NewLabelWithStyle(m.Name, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
				widget.NewLabel(m.Source),
			)))
	}

	nameE := widget.NewEntry()
	nameE.SetPlaceHolder("仓库名称")
	repoE := widget.NewEntry()
	repoE.SetPlaceHolder("github.com/owner/repo")
	box.Add(widget.NewForm(
		widget.NewFormItem("名称", nameE),
		widget.NewFormItem("地址", repoE),
	))
	box.Add(widget.NewButton("添加仓库", func() {
		if err := service.AddMarketplace(nameE.Text, repoE.Text); err != nil {
			dialog.ShowError(err, win)
			return
		}
		refresh()
	}))
	return box
}

// ── 更新 ──

func buildUpdateSettings(_ fyne.Window, _ func()) fyne.CanvasObject {
	box := container.NewVBox()
	box.Add(sectionTitle("更新"))
	box.Add(widget.NewLabel("Go 版是单文件程序，更新方式是下载新版本替换。"))
	box.Add(widget.NewSeparator())
	box.Add(widget.NewLabel("当前版本：v" + appVersion))
	box.Add(widget.NewLabel("发布地址：https://github.com/1696016910/CCWorkspace/releases"))
	box.Add(widget.NewLabel(""))
	box.Add(widget.NewLabel("更新包在 GitHub Releases 上，三个实现各有独立的产物："))
	box.Add(widget.NewLabel("  ClaudeWorkbench-Node-<版本>-x64.exe"))
	box.Add(widget.NewLabel("  ClaudeWorkbench-Go-<版本>-x64.exe"))
	box.Add(widget.NewLabel("  ClaudeWorkbench-Rust-<版本>-x64.exe"))
	return box
}

// ── 工具 ──

func joinLines(list []string) string {
	out := ""
	for i, s := range list {
		if i > 0 {
			out += "\n"
		}
		out += s
	}
	return out
}

func splitLines(text string) []string {
	out := []string{}
	cur := ""
	for _, r := range text {
		if r == '\n' {
			if t := trimSpace(cur); t != "" {
				out = append(out, t)
			}
			cur = ""
			continue
		}
		cur += string(r)
	}
	if t := trimSpace(cur); t != "" {
		out = append(out, t)
	}
	return out
}

func trimSpace(s string) string {
	start, end := 0, len(s)
	for start < end && (s[start] == ' ' || s[start] == '\t' || s[start] == '\r') {
		start++
	}
	for end > start && (s[end-1] == ' ' || s[end-1] == '\t' || s[end-1] == '\r') {
		end--
	}
	return s[start:end]
}

// 未使用但保留：后续做卡片高亮时用
var _ = color.White
