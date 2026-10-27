package service

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReadWorkspaceFileRejectsEscape(t *testing.T) {
	root := t.TempDir()
	outside := filepath.Join(t.TempDir(), "secret.txt")
	if err := os.WriteFile(outside, []byte("私钥"), 0o600); err != nil {
		t.Fatal(err)
	}

	// 用相对路径跳出工作区
	if _, err := ReadWorkspaceFile(root, "../secret.txt"); err == nil {
		t.Fatal("跳出工作区的相对路径必须被拒绝")
	}
	// 绝对路径指向工作区外
	if _, err := ReadWorkspaceFile(root, outside); err == nil {
		t.Fatal("工作区外的绝对路径必须被拒绝")
	}
}

func TestReadWorkspaceFileReadsInside(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "a.txt")
	if err := os.WriteFile(target, []byte("内容"), 0o600); err != nil {
		t.Fatal(err)
	}
	got, err := ReadWorkspaceFile(root, "a.txt")
	if err != nil {
		t.Fatalf("工作区内文件应可读: %v", err)
	}
	if got != "内容" {
		t.Fatalf("内容不符: %q", got)
	}
}

func TestReadWorkspaceFileRejectsBinary(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "a.png"), []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := ReadWorkspaceFile(root, "a.png")
	if err == nil || !strings.Contains(err.Error(), "二进制") {
		t.Fatalf("图片应提示无法以文本预览，实际: %v", err)
	}
}

func TestReadWorkspaceFileRejectsNonUTF8(t *testing.T) {
	root := t.TempDir()
	// GBK 编码的"中文"，不是合法 UTF-8
	if err := os.WriteFile(filepath.Join(root, "gbk.txt"), []byte{0xd6, 0xd0, 0xce, 0xc4}, 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := ReadWorkspaceFile(root, "gbk.txt")
	if err == nil || !strings.Contains(err.Error(), "UTF-8") {
		t.Fatalf("非 UTF-8 应明确报错，实际: %v", err)
	}
}
