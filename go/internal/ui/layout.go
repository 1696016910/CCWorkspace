package ui

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// ── 设计稿通用件 ──

// rect 返回一块纯色/描边矩形，用作分隔线或背景块。
func rect(c interface{ RGBA() (uint32, uint32, uint32, uint32) }) *canvas.Rectangle {
	r := canvas.NewRectangle(c)
	r.StrokeWidth = 0
	return r
}

// hairline 1px 分隔线。
func hairline() fyne.CanvasObject {
	l := canvas.NewRectangle(colLine)
	l.SetMinSize(fyne.NewSize(1, 1))
	return l
}

// text 返回一段静态文本。
func text(s string, size float32, c interface{ RGBA() (uint32, uint32, uint32, uint32) }) *canvas.Text {
	t := canvas.NewText(s, c)
	t.TextSize = size
	return t
}

// boldText 加粗静态文本。
func boldText(s string, size float32, c interface{ RGBA() (uint32, uint32, uint32, uint32) }) *canvas.Text {
	t := text(s, size, c)
	t.TextStyle = fyne.TextStyle{Bold: true}
	return t
}

// fill 把一个控件放进纯色背景容器，用来做圆角不可控时的替代。
func fill(c fyne.CanvasObject, bg interface{ RGBA() (uint32, uint32, uint32, uint32) }) *fyne.Container {
	bgRect := canvas.NewRectangle(bg)
	return container.NewMax(bgRect, c)
}

// ── 侧栏 ──

// sidebar 构建左侧栏：新建按钮 + 工作区/会话树 + 底部三按钮。
func (m *MainWindow) sidebar() fyne.CanvasObject {
	// .btn-new-ws height:36px radius:8px bg:green
	newBtn := widget.NewButton("＋  新建工作区", m.promptNewWorkspace)
	newBtn.Importance = widget.HighImportance
	top := container.NewPadded(newBtn)

	tree := container.NewVBox()
	m.wsBody = tree
	m.rebuildTree()

	// .side-foot
	foot := container.NewVBox(
		hairline(),
		m.footButton("◑", "暗色模式", func() { m.toggleDark() }),
		m.footButton("≡", "回收站", m.ShowTrash),
		m.footButton("⚙", "设置", m.ShowSettings),
	)

	list := container.NewVScroll(tree)
	return container.NewBorder(
		container.NewVBox(top, hairline()),
		container.NewPadded(foot),
		nil, nil,
		list,
	)
}

// footButton 一行图标 + 文字 + 可选角标（.foot-btn height:34px）。
func (m *MainWindow) footButton(icon, label string, onTap func()) fyne.CanvasObject {
	btn := widget.NewButton(icon+"   "+label, onTap)
	btn.Alignment = widget.ButtonAlignLeading
	btn.Importance = widget.LowImportance
	return btn
}

// rebuildTree 按当前数据重建工作区/会话树。
//
// 用 VBox 而不是 widget.List：设计稿要求工作区下的会话是嵌套缩进的，
// List 只管平铺行，做不出分组。
func (m *MainWindow) rebuildTree() {
	if m.wsBody == nil {
		return
	}
	m.wsBody.RemoveAll()
	if len(m.workspaces) == 0 {
		m.wsBody.Add(container.NewPadded(text("还没有工作区", fsFoot, colT4)))
		m.wsBody.Refresh()
		return
	}

	for _, w := range m.workspaces {
		w := w
		// .ws-head：三角 + 名称 + 数量 + ＋
		count := 0
		for _, s := range m.api.LoadSessions() {
			if s.WorkspaceID == w.ID {
				count++
			}
		}
		wsBtn := widget.NewButton("v   "+w.Name, func() {
			m.currentWS = w.ID
			m.currentSes = ""
			m.rebuildTree()
			m.refreshSessions()
		})
		wsBtn.Alignment = widget.ButtonAlignLeading
		wsBtn.Importance = widget.LowImportance
		if m.currentWS == w.ID {
			wsBtn.Text = "v   " + w.Name
		}
		addBtn := widget.NewButton("＋", func() {
			m.currentWS = w.ID
			m.newSession()
			m.rebuildTree()
		})
		addBtn.Importance = widget.LowImportance
		head := container.NewBorder(nil, nil, nil,
			container.NewHBox(text(itoa(count), fsWsCount, colT4), addBtn),
			wsBtn)

		m.wsBody.Add(container.NewPadded(head))
		m.wsBody.Add(hairline())

		// .chat-item margin-left:26px
		if m.currentWS == w.ID {
			found := false
			for _, s := range m.api.LoadSessions() {
				if s.WorkspaceID != w.ID {
					continue
				}
				found = true
				s := s
				title := s.Title
				if title == "" {
					title = "(新会话)"
				}
				label := "    " + title
				btn := widget.NewButton(label, func() {
					m.currentSes = s.ID
					m.loadChat()
					m.rebuildTree()
				})
				btn.Alignment = widget.ButtonAlignLeading
				btn.Importance = widget.LowImportance
				if m.currentSes == s.ID {
					btn.Importance = widget.HighImportance
				}
				row := container.NewHBox(text("  •", 9, colLine2), btn)
				m.wsBody.Add(row)
			}
			if !found {
				m.wsBody.Add(text("     暂无会话，点上方 ＋ 新建", fsChat, colT4))
			}
		}
	}
	m.wsBody.Refresh()
}

// toggleDark 切换明暗。Fyne 只能整体换主题变体。
func (m *MainWindow) toggleDark() {
	m.dark = !m.dark
	base := theme.LightTheme()
	if m.dark {
		base = theme.DarkTheme()
	}
	m.fyneApp.Settings().SetTheme(&appTheme{base: base, font: m.font})
}

// ── 主区 ──

// mainHead 顶部 48px 标题行：标题 + 工作区名 + 右侧图标按钮。
func (m *MainWindow) mainHead() fyne.CanvasObject {
	title := "(未选择会话)"
	if s := m.findSession(m.currentSes); s != nil {
		if s.Title == "" {
			title = "(新会话)"
		} else {
			title = s.Title
		}
	}
	titleT := boldText(title, fsMhTitle, colT1)

	wsName := ""
	if s := m.findSession(m.currentSes); s != nil {
		for _, w := range m.workspaces {
			if w.ID == s.WorkspaceID {
				wsName = w.Name
			}
		}
	}
	wsT := text("· "+wsName, fsMhWs, colT4)

	delBtn := widget.NewButton("删", func() {
		if s := m.findSession(m.currentSes); s != nil {
			t := s.Title
			if t == "" {
				t = "(新会话)"
			}
			m.askDelete("session", s.ID, t)
		} else {
			m.setStatus("请先选择一个会话")
		}
	})
	trashBtn := widget.NewButton("回", m.ShowTrash)
	setBtn := widget.NewButton("设", func() { m.ShowSettings() })
	for _, b := range []*widget.Button{delBtn, trashBtn, setBtn} {
		b.Importance = widget.LowImportance
	}

	left := container.NewHBox(titleT, wsT)
	head := container.NewBorder(nil, nil, left,
		container.NewHBox(delBtn, trashBtn, setBtn))
	return container.NewVBox(container.NewPadded(head), hairline())
}

// composer 输入区：圆角边框 + textarea + 底部工具栏。
func (m *MainWindow) composer() fyne.CanvasObject {
	m.input = widget.NewMultiLineEntry()
	m.input.SetPlaceHolder("描述你想让它做的事，或输入 / 调用命令与 Skill...")
	m.input.Wrapping = fyne.TextWrapWord

	m.sendBtn = widget.NewButton("↑ 发送", m.send)
	m.sendBtn.Importance = widget.HighImportance
	m.stopBtn = widget.NewButton("■ 停止", m.stop)
	m.stopBtn.Importance = widget.LowImportance
	m.stopBtn.Disable()

	m.ctxBtn = widget.NewButton("0%", m.showContext)
	m.ctxBtn.Importance = widget.LowImportance

	bar := container.NewBorder(nil, nil,
		container.NewHBox(widget.NewButton("＋", func() {})),
		container.NewHBox(m.sendBtn, m.stopBtn, m.ctxBtn),
	)

	box := container.NewBorder(nil, bar, nil, nil, m.input)
	pad := container.NewPadded(box)

	hint := container.NewCenter(text("Enter 发送 · Shift + Enter 换行", fsHint, colT4))
	return container.NewBorder(
		nil,
		container.NewVBox(pad, hint),
		nil, nil,
	)
}

// threadBox 对话流容器。
func (m *MainWindow) threadBox() fyne.CanvasObject {
	m.chatBox = container.NewVBox()
	m.chatScroll = container.NewVScroll(m.chatBox)
	return m.chatScroll
}

// mainContent 组装主区。
func (m *MainWindow) mainContentNew() fyne.CanvasObject {
	return container.NewBorder(m.mainHead(), nil, nil, nil, m.threadBox())
}
