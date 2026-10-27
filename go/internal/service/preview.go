package service

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

// PreviewMaxBytes 预览文件大小上限。超过则拒绝，避免一次性读入巨大文件。
const PreviewMaxBytes = 2 * 1024 * 1024

// binaryExts 这些扩展名不可能以文本方式有意义地展示。
var binaryExts = map[string]bool{
	".png": true, ".jpg": true, ".jpeg": true, ".gif": true, ".webp": true,
	".ico": true, ".bmp": true, ".pdf": true, ".zip": true, ".gz": true,
	".7z": true, ".rar": true, ".exe": true, ".dll": true, ".so": true,
	".woff": true, ".woff2": true, ".ttf": true, ".mp3": true, ".mp4": true,
	".mov": true, ".dylib": true, ".node": true, ".dmg": true, ".pkg": true,
	".deb": true, ".appimage": true,
}

// ReadWorkspaceFile 读取工作区内的文本文件用于预览。
//
// 预览入口来自模型输出，属于不可信内容，必须限制在工作区内：
// 否则一个指向 ~/.ssh/id_rsa 的链接点一下就把私钥显示在界面上。
// 字面路径和真实路径都要检查，工作区内可以放 junction 指向工作区外。
func ReadWorkspaceFile(root, rel string) (string, error) {
	if strings.TrimSpace(rel) == "" {
		return "", errors.New("路径不能为空")
	}
	// UNC（\\host\share）会向远端发起 SMB 认证，可能外泄 NTLM 凭据
	if strings.HasPrefix(rel, `\\`) || strings.HasPrefix(rel, "//") {
		return "", errors.New("不支持网络路径")
	}

	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("工作区路径无效: %w", err)
	}
	target := rel
	if !filepath.IsAbs(target) {
		target = filepath.Join(rootAbs, rel)
	}
	target, err = filepath.Abs(target)
	if err != nil {
		return "", fmt.Errorf("路径无效: %w", err)
	}
	if !within(target, rootAbs) {
		return "", errors.New("只能预览当前工作区内的文件")
	}

	// 再解析一次真实路径：工作区内可以放一个指向 ~/.claude 的 junction，
	// 字面路径检查会放行，实际读到的却是工作区外的文件。
	realTarget, err := filepath.EvalSymlinks(target)
	if err != nil {
		return "", errors.New("文件不存在或无法访问")
	}
	realRoot, err := filepath.EvalSymlinks(rootAbs)
	if err != nil {
		realRoot = rootAbs
	}
	if !within(realTarget, realRoot) {
		return "", errors.New("该路径指向工作区外，已阻止预览")
	}

	info, err := os.Stat(realTarget)
	if err != nil {
		return "", errors.New("文件不存在或无法访问")
	}
	if info.IsDir() {
		return "", errors.New("该路径不是文件")
	}
	if info.Size() > PreviewMaxBytes {
		return "", fmt.Errorf("文件过大（%d KB），暂不支持预览", info.Size()/1024)
	}
	if binaryExts[strings.ToLower(filepath.Ext(realTarget))] {
		return "", errors.New("这是二进制文件，无法以文本方式预览")
	}

	data, err := os.ReadFile(realTarget)
	if err != nil {
		return "", errors.New("读取失败：文件无法访问")
	}
	// UTF-8 BOM
	if len(data) >= 3 && data[0] == 0xef && data[1] == 0xbb && data[2] == 0xbf {
		data = data[3:]
	}
	if !utf8.Valid(data) {
		return "", errors.New("文件不是 UTF-8 文本，暂不支持预览")
	}
	return string(data), nil
}

// within 判断 p 是否在 root 内（或等于 root）。Windows 下忽略大小写。
func within(p, root string) bool {
	p = filepath.Clean(p)
	root = filepath.Clean(root)
	if isWindows() {
		p, root = strings.ToLower(p), strings.ToLower(root)
	}
	if p == root {
		return true
	}
	sep := string(filepath.Separator)
	return strings.HasPrefix(p, root+sep)
}
