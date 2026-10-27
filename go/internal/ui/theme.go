package ui

import (
	"fyne.io/fyne/v2"
	"image/color"
	"os"
	"strconv"
)

// cjkTheme 在默认主题上替换字体，让中文正常显示。
type cjkTheme struct {
	base fyne.Theme
	font fyne.Resource
}

func (t *cjkTheme) Color(name fyne.ThemeColorName, v fyne.ThemeVariant) color.Color {
	return t.base.Color(name, v)
}

func (t *cjkTheme) Font(style fyne.TextStyle) fyne.Resource {
	return t.font
}

func (t *cjkTheme) Icon(name fyne.ThemeIconName) fyne.Resource {
	return t.base.Icon(name)
}

func (t *cjkTheme) Size(name fyne.ThemeSizeName) float32 {
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
