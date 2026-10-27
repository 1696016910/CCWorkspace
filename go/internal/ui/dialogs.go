package ui

import (
	"claude-workbench/internal/model"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// ── 回收站 ──

// ShowTrash 打开回收站。删除的会话在这里可以恢复或彻底删除。
func (m *MainWindow) ShowTrash() {
	list := m.api.DeletedSessions()
	box := container.NewVBox()
	box.Add(widget.NewLabel("删除的会话会先放在这里，可以随时恢复。彻底删除后无法找回。"))
	box.Add(widget.NewSeparator())

	if len(list) == 0 {
		box.Add(widget.NewLabel("回收站是空的"))
	}
	for _, s := range list {
		s := s
		title := s.Title
		if title == "" {
			title = "(新会话)"
		}
		row := container.NewBorder(nil, nil, nil,
			container.NewHBox(
				widget.NewButton("恢复", func() {
					if err := m.api.RestoreSession(s.ID); err != nil {
						dialog.ShowError(err, m.win)
						return
					}
					m.ShowTrash()
					m.refreshSessions()
					m.setStatus("已恢复会话")
				}),
				widget.NewButton("彻底删除", func() {
					dialog.ShowConfirm("彻底删除会话",
						"确定彻底删除「"+title+"」？\n会话与全部消息都会被删除，无法找回。",
						func(ok bool) {
							if !ok {
								return
							}
							if err := m.api.PurgeSession(s.ID); err != nil {
								dialog.ShowError(err, m.win)
								return
							}
							m.ShowTrash()
							m.setStatus("已彻底删除")
						}, m.win)
				}),
			),
			widget.NewLabelWithStyle(title, fyne.TextAlignLeading, fyne.TextStyle{Bold: true}),
		)
		box.Add(row)
		box.Add(widget.NewLabel(fmt.Sprintf("%s · %d 条消息", friendlyTime(s.UpdatedAt), s.MessageCount)))
		box.Add(widget.NewSeparator())
	}

	d := dialog.NewCustom("回收站", "关闭", container.NewVScroll(box), m.win)
	d.Resize(fyne.NewSize(640, 560))
	d.Show()
}

// ── 上下文用量 ──

// showContext 显示当前会话的上下文用量。
//
// 只展示已落库的快照，不额外拉起一次 /context：那会额外消耗一轮 tokens。
func (m *MainWindow) showContext() {
	s := m.findSession(m.currentSes)
	if s == nil {
		dialog.ShowInformation("上下文", "请先选择一个会话", m.win)
		return
	}
	pct := s.ContextPercent
	head := widget.NewLabelWithStyle(
		fmt.Sprintf("%.1f%%", pct),
		fyne.TextAlignCenter,
		fyne.TextStyle{Bold: true},
	)
	bar := widget.NewProgressBar()
	bar.SetValue(clamp01(pct / 100))
	bar.TextFormatter = func() string { return "" } // 百分比已在上面大字显示

	box := container.NewVBox(
		widget.NewLabel("上下文使用情况"),
		widget.NewSeparator(),
		head,
		bar,
		widget.NewLabel(fmt.Sprintf("已用 %s tokens", humanTokens(s.ContextTokens))),
		widget.NewSeparator(),
		widget.NewLabel("每轮对话结束后由 CLI 的用量事件更新。"),
	)
	d := dialog.NewCustom("上下文", "关闭", box, m.win)
	d.Resize(fyne.NewSize(480, 320))
	d.Show()
}

func (m *MainWindow) findSession(id string) *model.SessionSummary {
	for i := range m.sessions {
		if m.sessions[i].ID == id {
			return &m.sessions[i]
		}
	}
	return nil
}

// friendlyTime 把库里的 ISO 时间转成本地「今天 14:03」这类可读形式。
//
// 解析失败时原样返回，不因为格式异常把界面弄空。
func friendlyTime(iso string) string {
	t, err := time.Parse(time.RFC3339, iso)
	if err != nil {
		return iso
	}
	local := t.Local()
	now := time.Now()
	if local.Format("2006-01-02") == now.Format("2006-01-02") {
		return "今天 " + local.Format("15:04")
	}
	if local.Format("2006-01-02") == now.AddDate(0, 0, -1).Format("2006-01-02") {
		return "昨天 " + local.Format("15:04")
	}
	if local.Year() == now.Year() {
		return local.Format("01-02 15:04")
	}
	return local.Format("2006-01-02")
}

func clamp01(v float64) float64 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

func humanTokens(n int) string {
	switch {
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fM", float64(n)/1_000_000)
	case n >= 1000:
		return fmt.Sprintf("%.1fk", float64(n)/1000)
	default:
		return itoa(n)
	}
}

// ── 文件预览 ──

// previewFile 在工作区内打开一个文本文件预览。
func (m *MainWindow) previewFile(rel string) {
	ws := m.currentWorkspace()
	if ws == nil {
		dialog.ShowInformation("预览", "请先选择一个工作区", m.win)
		return
	}
	content, err := m.api.ReadFile(ws.Path, rel)
	if err != nil {
		dialog.ShowError(err, m.win)
		return
	}

	text := widget.NewLabel(content)
	text.Wrapping = fyne.TextWrapWord
	text.TextStyle = fyne.TextStyle{Monospace: true}

	d := dialog.NewCustom(rel, "关闭", container.NewVScroll(text), m.win)
	d.Resize(fyne.NewSize(860, 660))
	d.Show()
}

func (m *MainWindow) currentWorkspace() *model.Workspace {
	for i := range m.workspaces {
		if m.workspaces[i].ID == m.currentWS {
			return &m.workspaces[i]
		}
	}
	return nil
}

// looksLikePath 判断工具调用的目标是否像文件路径。
//
// 只有路径才值得提供预览入口；命令、URL、搜索模式点击无意义。
func looksLikePath(s string) bool {
	if s == "" || strings.HasPrefix(s, "http://") || strings.HasPrefix(s, "https://") {
		return false
	}
	if strings.Contains(s, "\n") || (strings.Contains(s, " ") && !strings.Contains(s, "/")) {
		return false
	}
	ext := strings.ToLower(filepath.Ext(s))
	if ext == "" {
		return false
	}
	return !binaryExts[ext]
}

// binaryExts 与 service 包保持一致，避免把图片当文本预览。
var binaryExts = map[string]bool{
	".png": true, ".jpg": true, ".jpeg": true, ".gif": true, ".webp": true,
	".ico": true, ".bmp": true, ".pdf": true, ".zip": true, ".gz": true,
	".7z": true, ".rar": true, ".exe": true, ".dll": true, ".so": true,
	".woff": true, ".woff2": true, ".ttf": true, ".mp3": true, ".mp4": true,
	".mov": true, ".dylib": true, ".node": true, ".dmg": true, ".pkg": true,
}

// fileChip 为带文件路径的工具调用生成一个可点击的预览入口。
func (m *MainWindow) fileChip(target string) fyne.CanvasObject {
	b := widget.NewButtonWithIcon(target, theme.DocumentIcon(), func() {
		m.previewFile(target)
	})
	b.Alignment = widget.ButtonAlignLeading
	return b
}
