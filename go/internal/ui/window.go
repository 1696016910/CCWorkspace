// Package ui 提供 Fyne 界面。
package ui

import (
	"claude-workbench/internal/model"
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
	"log"
	"time"
)

// MainWindow 主窗口状态。
type MainWindow struct {
	fyneApp fyne.App
	win     fyne.Window

	workspaces []model.Workspace
	sessions   []model.SessionSummary
	currentWS  string
	currentSes string

	// 左侧栏：工作区 + 会话
	wsList     *widget.List
	sessList   *widget.List
	wsNames    []string
	sessTitles []string

	// 对话区
	chatBox *fyne.Container
	chatScroll *container.Scroll

	// 输入
	input   *widget.Entry
	sendBtn *widget.Button
	stopBtn *widget.Button

	// 状态
	statusLabel *widget.Label
	steaming    bool

	// 注入的业务层
	api  API
	chat ChatFn

	// 主界面根容器，设置页打开时暂存以便返回
	root fyne.CanvasObject

	// 待确认的删除："session" 或 "workspace"
	pendingKind string
	pendingID   string
	pendingName string
}

// NewMainWindow 构建主窗口。
func NewMainWindow(api API, chat ChatFn) *MainWindow {
	a := app.NewWithID("com.jianzhou.claude-workbench.go")

	// 加载系统中文字体：Fyne 内置字体没有中文，不加载会显示方框
	if p := findChineseFont(); p != "" {
		if res, err := loadFontResource(p); err == nil {
			a.Settings().SetTheme(&cjkTheme{base: theme.DefaultTheme(), font: res})
		} else {
			log.Printf("加载中文字体失败: %v", err)
		}
	} else {
		log.Printf("未找到中文字体，界面中文可能显示为方框")
	}

	w := a.NewWindow("Claude 工作台")
	w.Resize(fyne.NewSize(1200, 800))

	m := &MainWindow{fyneApp: a, win: w, api: api, chat: chat}
	m.build()
	return m
}

// Run 进入事件循环。
func (m *MainWindow) Run() { m.win.ShowAndRun() }

func (m *MainWindow) build() {
	m.workspaces = m.api.LoadWorkspaces()
	m.sessions = m.api.LoadSessions()
	m.refreshNames()

	// ── 左侧栏 ──
	wsList := widget.NewList(
		func() int { return len(m.wsNames) },
		func() fyne.CanvasObject {
			return widget.NewLabel("")
		},
		func(i widget.ListItemID, o fyne.CanvasObject) {
			o.(*widget.Label).SetText(m.wsNames[i])
		},
	)
	wsList.OnSelected = func(i widget.ListItemID) {
		if i < 0 || i >= len(m.workspaces) {
			return
		}
		m.currentWS = m.workspaces[i].ID
		m.refreshSessions()
	}
	m.wsList = wsList

	sessList := widget.NewList(
		func() int { return len(m.sessTitles) },
		func() fyne.CanvasObject { return widget.NewLabel("") },
		func(i widget.ListItemID, o fyne.CanvasObject) {
			o.(*widget.Label).SetText(m.sessTitles[i])
		},
	)
	sessList.OnSelected = func(i widget.ListItemID) {
		if i < 0 || i >= len(m.sessions) {
			return
		}
		m.currentSes = m.sessions[i].ID
		m.loadChat()
	}
	m.sessList = sessList

	newWsBtn := widget.NewButtonWithIcon("新建工作区", theme.ContentAddIcon(), m.promptNewWorkspace)
	newSessBtn := widget.NewButtonWithIcon("新建会话", theme.ContentAddIcon(), m.newSession)

	// 删除入口：选中项后点删除，先弹确认
	delWsBtn := widget.NewButton("删除工作区", func() {
		if m.currentWS == "" {
			m.setStatus("请先选择一个工作区")
			return
		}
		for _, w := range m.workspaces {
			if w.ID == m.currentWS {
				m.askDelete("workspace", w.ID, w.Name)
				return
			}
		}
	})
	delSesBtn := widget.NewButton("删除会话", func() {
		if m.currentSes == "" {
			m.setStatus("请先选择一个会话")
			return
		}
		for _, s := range m.sessions {
			if s.ID == m.currentSes {
				title := s.Title
				if title == "" {
					title = "(新会话)"
				}
				m.askDelete("session", s.ID, title)
				return
			}
		}
	})

	// 四个会话操作收进一个「更多」菜单：纵向堆叠会挤掉会话列表的可见行数。
	var moreSesBtn *widget.Button
	moreSesBtn = widget.NewButtonWithIcon("更多", theme.MoreHorizontalIcon(), func() {
		s := m.findSession(m.currentSes)
		if s == nil {
			m.setStatus("请先选择一个会话")
			return
		}
		pinLabel := "置顶"
		if s.PinnedAt != nil {
			pinLabel = "取消置顶"
		}
		menu := fyne.NewMenu("",
			fyne.NewMenuItem("重命名", func() { m.promptRenameSession(s.ID, s.Title) }),
			fyne.NewMenuItem(pinLabel, func() {
				if err := m.api.PinSession(s.ID, s.PinnedAt == nil); err != nil {
					m.setStatus(err.Error())
					return
				}
				m.refreshSessions()
				m.setStatus("已更新置顶")
			}),
			fyne.NewMenuItem("复制会话", func() {
				if _, err := m.api.DuplicateSession(s.ID); err != nil {
					m.setStatus(err.Error())
					return
				}
				m.refreshSessions()
				m.setStatus("已复制会话")
			}),
			fyne.NewMenuItemSeparator(),
			fyne.NewMenuItem("删除会话", func() {
				title := s.Title
				if title == "" {
					title = "(新会话)"
				}
				m.askDelete("session", s.ID, title)
			}),
		)
		widget.ShowPopUpMenuAtPosition(menu, m.win.Canvas(), fyne.CurrentApp().Driver().AbsolutePositionForObject(moreSesBtn))
	})

	wsPane := container.NewBorder(
		container.NewVBox(newWsBtn, delWsBtn, widget.NewLabel("工作区")),
		nil, nil, nil, wsList)
	sessPane := container.NewBorder(
		container.NewVBox(container.NewBorder(nil, nil, nil,
			container.NewHBox(moreSesBtn, delSesBtn), newSessBtn), widget.NewLabel("会话")),
		nil, nil, nil, sessList)

	// 上下两半，各自带滚动；固定最小宽度避免被右侧对话区挤压
	sidebar := container.NewVSplit(wsPane, sessPane)
	sidebar.SetOffset(0.34)
	sidebarBox := container.NewGridWrap(fyne.NewSize(250, 700), sidebar)

	// ── 对话区 ──
	m.chatBox = container.NewVBox()
	m.chatScroll = container.NewVScroll(m.chatBox)
	m.chatScroll.SetMinSize(fyne.NewSize(600, 500))

	// ── 输入区 ──
	m.input = widget.NewMultiLineEntry()
	m.input.SetPlaceHolder("描述你想让它做的事")
	m.input.Wrapping = fyne.TextWrapWord

	m.sendBtn = widget.NewButtonWithIcon("发送", theme.MailSendIcon(), m.send)
	m.stopBtn = widget.NewButtonWithIcon("停止", theme.MediaStopIcon(), m.stop)
	m.stopBtn.Disable()

	m.statusLabel = widget.NewLabel("就绪")

	contextBtn := widget.NewButtonWithIcon("上下文", theme.InfoIcon(), m.showContext)
	trashBtn := widget.NewButtonWithIcon("回收站", theme.DeleteIcon(), m.ShowTrash)

	composer := container.NewBorder(
		nil,
		container.NewHBox(m.sendBtn, m.stopBtn, contextBtn, trashBtn, m.statusLabel),
		nil, nil,
		m.input,
	)

	head := widget.NewLabel("Claude 工作台 · Go 版")
	head.TextStyle = fyne.TextStyle{Bold: true}
	settingsBtn := widget.NewButtonWithIcon("设置", theme.SettingsIcon(), func() {
		m.ShowSettings()
	})

	right := container.NewBorder(
		container.NewBorder(nil, nil, nil, settingsBtn, head), composer, nil, nil,
		m.chatScroll,
	)

	split := container.NewHSplit(sidebarBox, right)
	split.SetOffset(0.22)

	m.root = split
	m.win.SetContent(split)

	m.refreshSessions()
	m.appendSystem("已就绪。数据库：" + m.api.Summary())
}

// mainContent 返回主界面，供从设置页返回时恢复。
func (m *MainWindow) mainContent() fyne.CanvasObject { return m.root }

// refreshNames 重建左侧显示文本。
func (m *MainWindow) refreshNames() {
	m.wsNames = make([]string, 0, len(m.workspaces))
	for _, w := range m.workspaces {
		pin := ""
		if w.PinnedAt != nil {
			pin = "• "
		}
		m.wsNames = append(m.wsNames, pin+w.Name)
	}
	if m.wsList != nil {
		m.wsList.Refresh()
	}
}

// refreshSessions 按当前工作区过滤会话。
func (m *MainWindow) refreshSessions() {
	m.sessions = m.api.LoadSessions()
	m.sessTitles = m.sessTitles[:0]
	filtered := make([]model.SessionSummary, 0, len(m.sessions))
	for _, s := range m.sessions {
		if m.currentWS != "" && s.WorkspaceID != m.currentWS {
			continue
		}
		title := s.Title
		if title == "" {
			title = "(新会话)"
		}
		pin := ""
		if s.PinnedAt != nil {
			pin = "• "
		}
		m.sessTitles = append(m.sessTitles, pin+title)
		filtered = append(filtered, s)
	}
	m.sessions = filtered
	if m.sessList != nil {
		m.sessList.Refresh()
	}
}

// loadChat 读取并渲染一个会话的消息。
func (m *MainWindow) loadChat() {
	m.chatBox.RemoveAll()
	msgs := m.api.LoadMessages(m.currentSes)
	for _, msg := range msgs {
		m.appendMessage(msg)
	}
	m.chatScroll.ScrollToBottom()
}

func (m *MainWindow) appendMessage(msg model.ChatMessage) {
	text := widget.NewLabel(msg.Text)
	text.Wrapping = fyne.TextWrapWord

	if msg.Role == "user" {
		text.TextStyle = fyne.TextStyle{Bold: true}
	}
	m.chatBox.Add(text)

	if msg.Stopped {
		hint := widget.NewLabel("■ 已停止生成")
		hint.TextStyle = fyne.TextStyle{Italic: true}
		m.chatBox.Add(hint)
	}

	if len(msg.Tools) > 0 {
		m.chatBox.Add(widget.NewLabel(m.toolSummary(msg.Tools)))
		for _, t := range msg.Tools {
			if looksLikePath(t.Target) {
				m.chatBox.Add(m.fileChip(t.Target))
			}
		}
	}
	m.chatBox.Refresh()
}

func (m *MainWindow) toolSummary(tools []model.ToolCall) string {
	s := "已处理 " + itoa(len(tools)) + " 步："
	for i, t := range tools {
		if i > 0 {
			s += " · "
		}
		s += t.Name
	}
	return s
}

func (m *MainWindow) appendSystem(text string) {
	l := widget.NewLabel(text)
	l.Wrapping = fyne.TextWrapWord
	l.TextStyle = fyne.TextStyle{Italic: true}
	m.chatBox.Add(l)
	m.chatBox.Refresh()
}

// ── 交互 ──

func (m *MainWindow) promptNewWorkspace() {
	pathEntry := widget.NewEntry()
	pathEntry.SetPlaceHolder("项目目录绝对路径")
	nameEntry := widget.NewEntry()
	nameEntry.SetPlaceHolder("工作区名称（留空用目录名）")

	items := []*widget.FormItem{
		widget.NewFormItem("目录", pathEntry),
		widget.NewFormItem("名称", nameEntry),
	}
	dialog.ShowForm("新建工作区", "创建", "取消", items, func(ok bool) {
		if !ok {
			return
		}
		list, err := m.api.AddWorkspace(pathEntry.Text, nameEntry.Text)
		if err != nil {
			dialog.ShowError(err, m.win)
			return
		}
		m.workspaces = list
		m.refreshNames()
	}, m.win)
}

// promptRenameSession 弹窗重命名会话。
func (m *MainWindow) promptRenameSession(id, title string) {
	entry := widget.NewEntry()
	entry.SetText(title)
	dialog.ShowForm("重命名会话", "保存", "取消",
		[]*widget.FormItem{widget.NewFormItem("标题", entry)},
		func(ok bool) {
			if !ok {
				return
			}
			if err := m.api.RenameSession(id, entry.Text); err != nil {
				dialog.ShowError(err, m.win)
				return
			}
			m.refreshSessions()
			m.setStatus("已重命名")
		}, m.win)
}

// setStatus 更新状态栏，供回调里使用。
func (m *MainWindow) setStatus(text string) {
	m.statusLabel.SetText(text)
}

// askDelete 弹出删除确认。
func (m *MainWindow) askDelete(kind, id, name string) {
	m.pendingKind, m.pendingID, m.pendingName = kind, id, name
	label := "会话"
	tone := "移到回收站，可以随时恢复"
	if kind == "workspace" {
		label = "工作区"
		tone = "将同时删除它下面的所有会话与消息，且无法恢复。磁盘目录不会被删除"
	}
	dialog.ShowConfirm(
		"删除"+label,
		"确定删除「"+name+"」？\n"+tone,
		func(ok bool) {
			if !ok {
				m.pendingKind = ""
				return
			}
			m.confirmDelete()
		}, m.win)
}

func (m *MainWindow) confirmDelete() {
	kind, id := m.pendingKind, m.pendingID
	m.pendingKind = ""
	switch kind {
	case "workspace":
		if _, err := m.api.RemoveWorkspace(id); err != nil {
			m.setStatus(err.Error())
			return
		}
		if m.currentWS == id {
			m.currentWS = ""
		}
		m.workspaces = m.api.LoadWorkspaces()
		m.refreshNames()
		m.refreshSessions()
		m.setStatus("已删除工作区")
	case "session":
		if err := m.api.DeleteSession(id); err != nil {
			m.setStatus(err.Error())
			return
		}
		if m.currentSes == id {
			m.currentSes = ""
			m.chatBox.RemoveAll()
		}
		m.refreshSessions()
		m.setStatus("已移到回收站")
	}
}

func (m *MainWindow) newSession() {
	if m.currentWS == "" {
		dialog.ShowInformation("提示", "请先选择一个工作区", m.win)
		return
	}
	if _, err := m.api.NewSession(m.currentWS); err != nil {
		dialog.ShowError(err, m.win)
		return
	}
	m.refreshSessions()
}

func (m *MainWindow) send() {
	text := m.input.Text
	if text == "" || m.currentSes == "" || m.steaming {
		return
	}
	m.input.SetText("")
	m.appendMessage(model.ChatMessage{Role: "user", Text: text})
	m.steaming = true
	m.sendBtn.Disable()
	m.stopBtn.Enable()
	m.statusLabel.SetText("正在生成…")

	// 实际对话逻辑在 chat 桥接层，这里只保证 UI 状态正确
	go func() {
		time.Sleep(50 * time.Millisecond) // 让界面先刷新出用户消息
		m.streamReply(text)
	}()
}

func (m *MainWindow) streamReply(text string) {
	if m.chat == nil {
		m.finishStream("（对话功能未接入）", false)
		return
	}
	m.chat(m.currentSes, text,
		func(delta string) {
			// 追加到最后一个标签
			fyne.Do(func() {
				if n := len(m.chatBox.Objects); n > 0 {
					if l, ok := m.chatBox.Objects[n-1].(*widget.Label); ok {
						l.SetText(l.Text + delta)
						return
					}
				}
				l := widget.NewLabel(delta)
				l.Wrapping = fyne.TextWrapWord
				m.chatBox.Add(l)
			})
		},
		func(full string, stopped bool) {
			fyne.Do(func() {
				m.finishStream(full, stopped)
				// 结束后重载历史，让界面与数据库一致
				if m.currentSes != "" {
					m.loadChat()
				}
			})
		})
}

func (m *MainWindow) finishStream(_ string, stopped bool) {
	m.steaming = false
	m.sendBtn.Enable()
	m.stopBtn.Disable()
	if stopped {
		m.statusLabel.SetText("已停止")
		m.appendSystem("■ 已停止生成")
	} else {
		m.statusLabel.SetText("就绪")
	}
	m.chatScroll.ScrollToBottom()
}

func (m *MainWindow) stop() {
	if !m.steaming {
		return
	}
	m.statusLabel.SetText("正在停止…")
	// 实际中断由 chat 桥接层处理
	m.finishStream("", true)
}

func (m *MainWindow) refreshChat() { m.chatScroll.Refresh() }

// Fatal 显示致命错误后退出。
func Fatal(title, msg string) {
	a := app.New()
	w := a.NewWindow(title)
	w.SetContent(container.NewVBox(
		canvas.NewText(title, theme.ErrorColor()),
		widget.NewLabel(msg),
	))
	w.ShowAndRun()
}
