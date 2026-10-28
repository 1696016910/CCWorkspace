package ui

import (
	"image/color"
	"os"
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/theme"
)

// 设计令牌，数值来自 prototype/20261027-界面原型.html 的 CSS 变量。
// 改动前先对照那份稿子，别凭感觉调颜色和间距。
var (
	colBg      = color.NRGBA{0xff, 0xff, 0xff, 0xff}
	colBgBar   = color.NRGBA{0xfa, 0xfa, 0xfa, 0xff}
	colHover   = color.NRGBA{0xf0, 0xf0, 0xf1, 0xff}
	colSoft    = color.NRGBA{0xf5, 0xf5, 0xf6, 0xff}
	colSelect  = color.NRGBA{0xe7, 0xe7, 0xe9, 0xff}
	colLine    = color.NRGBA{0xe6, 0xe6, 0xe8, 0xff}
	colLine2   = color.NRGBA{0xdc, 0xdc, 0xde, 0xff}
	colT1      = color.NRGBA{0x1f, 0x1f, 0x23, 0xff}
	colT2      = color.NRGBA{0x5f, 0x5f, 0x66, 0xff}
	colT3      = color.NRGBA{0x62, 0x62, 0x6a, 0xff}
	colT4      = color.NRGBA{0x6f, 0x6f, 0x78, 0xff}
	colGreen   = color.NRGBA{0x47, 0x81, 0x5c, 0xff}
	colGreenH  = color.NRGBA{0x3f, 0x75, 0x53, 0xff}
	colGreenS  = color.NRGBA{0xed, 0xf4, 0xef, 0xff}
	colRed     = color.NRGBA{0xc0, 0x39, 0x2b, 0xff}
	colWhite   = color.NRGBA{0xff, 0xff, 0xff, 0xff}
	colTrans   = color.NRGBA{0, 0, 0, 0}
	colDangerB = color.NRGBA{0xfd, 0xf5, 0xf5, 0xff}
	colDangerL = color.NRGBA{0xf2, 0xde, 0xdd, 0xff}
	colDangerT = color.NRGBA{0xa3, 0x3c, 0x34, 0xff}
)

// 尺寸（设计稿 px）
const (
	sizeSidebarW    = 246
	sizeNewWsH      = 36
	sizeWsRowH      = 30
	sizeChatRowH    = 30
	sizeFootBtnH    = 34
	sizeHeadH       = 48
	radiusNewWs     = 8
	radiusRow       = 7
	radiusBubble    = 14
	radiusComposer  = 14
	roundingCardPad = 12
)

// 字号
const (
	fsNewWs   = 13
	fsWsName  = 13
	fsWsCount = 11
	fsChat    = 12.5
	fsFoot    = 12.5
	fsMhTitle = 14
	fsMhWs    = 11.5
	fsBubble  = 13.5
	fsAiText  = 13.5
	fsRole    = 11.5
	fsStopped = 12
	fsHint    = 11
)

// appTheme 用设计稿的配色覆盖 Fyne 默认主题。
type appTheme struct {
	base fyne.Theme
	font fyne.Resource
}

func (t *appTheme) Color(name fyne.ThemeColorName, v fyne.ThemeVariant) color.Color {
	switch name {
	case theme.ColorNameBackground, theme.ColorNameInputBackground, theme.ColorNameMenuBackground, theme.ColorNameOverlayBackground:
		return colBg
	case theme.ColorNameForeground:
		return colT1
	case theme.ColorNamePlaceHolder, theme.ColorNameDisabled:
		return colT4
	case theme.ColorNamePrimary, theme.ColorNameFocus, theme.ColorNameSelection:
		return colGreen
	case theme.ColorNameHover, theme.ColorNamePressed:
		return colHover
	case theme.ColorNameSeparator:
		return colLine
	case theme.ColorNameButton:
		return colSoft
	case theme.ColorNameInputBorder:
		return colLine2
	case theme.ColorNameError:
		return colRed
	case theme.ColorNameSuccess:
		return colGreen
	case theme.ColorNameWarning:
		return colDangerT
	case theme.ColorNameShadow:
		return color.NRGBA{0, 0, 0, 0x1c}
	}
	return t.base.Color(name, v)
}

func (t *appTheme) Font(style fyne.TextStyle) fyne.Resource { return t.font }

func (t *appTheme) Icon(name fyne.ThemeIconName) fyne.Resource { return t.base.Icon(name) }

func (t *appTheme) Size(name fyne.ThemeSizeName) float32 {
	switch name {
	case theme.SizeNameText:
		return fsAiText
	case theme.SizeNameInputBorder:
		return 1
	case theme.SizeNameScrollBar:
		return 8
	case theme.SizeNameScrollBarSmall:
		return 4
	case theme.SizeNamePadding:
		return 4
	}
	return t.base.Size(name)
}

// loadFontResource 把字体文件读成 Fyne 资源。
func loadFontResource(path string) (fyne.Resource, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return fyne.NewStaticResource("cjk.ttf", data), nil
}

func itoa(n int) string { return strconv.Itoa(n) }
