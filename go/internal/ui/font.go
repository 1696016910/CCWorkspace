package ui

import (
	"os"
	"path/filepath"
	"runtime"
)

// 候选中文字体。Fyne 内置只带拉丁字形，不加载中文字体会显示成方框。
// 注意：Fyne 只接受单字体文件，.ttc 是字体集合，加载会报
// "collections not allowed"。所以优先用 .ttf。
var fontCandidates = []string{
	`C:\Windows\Fonts\Deng.ttf`,     // 等线（Windows 10+ 自带）
	`C:\Windows\Fonts\simhei.ttf`,   // 黑体
	`C:\Windows\Fonts\msyh.ttf`,     // 旧版微软雅黑单文件
	`/System/Library/Fonts/PingFang.ttf`,
	`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttf`,
	`/usr/share/fonts/truetype/wqy/wqy-microhei.ttf`,
}

// findChineseFont 返回第一个存在的中文字体路径。
// 找不到时返回空串，界面会退化为拉丁字体（中文显示为方框）。
func findChineseFont() string {
	for _, p := range fontCandidates {
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	// 兜底：在系统字体目录里按关键字找
	dir := "/usr/share/fonts"
	if runtime.GOOS == "windows" {
		dir = `C:\Windows\Fonts`
	}
	matches, _ := filepath.Glob(filepath.Join(dir, "*.ttf"))
	for _, m := range matches {
		base := filepath.Base(m)
		for _, kw := range []string{"msyh", "deng", "simhei", "noto", "wqy", "PingFang"} {
			if len(base) >= len(kw) && contains(base, kw) {
				return m
			}
		}
	}
	return ""
}

func contains(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
