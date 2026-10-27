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

	wsPane := container.NewBorder(
		container.NewVBox(newWsBtn, widget.NewLabel("工作区")),
		nil, nil, nil, wsList)
	sessPane := container.NewBorder(
		container.NewVBox(newSessBtn, widget.NewLabel("会话")),
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

	composer := container.NewBorder(
		nil,
		container.NewHBox(m.sendBtn, m.stopBtn, m.statusLabel),
		nil, nil,
		m.input,
	)

	head := widget.NewLabel("Claude 工作台 · Go 版")
	head.TextStyle = fyne.TextStyle{Bold: true}

	right := container.NewBorder(
		head, composer, nil, nil,
		m.chatScroll,
	)

	split := container.NewHSplit(sidebarBox, right)
	split.SetOffset(0.22)

	m.win.SetContent(split)

	m.refreshSessions()
	m.appendSystem("已就绪。数据库：" + m.api.Summary())
}

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
