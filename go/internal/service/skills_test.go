package service

import (
	"os"
	"path/filepath"
	"testing"
)

func makeSkill(t *testing.T, home, dir, name, desc string) {
	t.Helper()
	p := filepath.Join(home, ".claude", "skills", dir)
	if err := os.MkdirAll(p, 0o755); err != nil {
		t.Fatal(err)
	}
	body := "---\nname: " + name + "\ndescription: " + desc + "\n---\n\n正文"
	if err := os.WriteFile(filepath.Join(p, "SKILL.md"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestListSkills(t *testing.T) {
	home := withTempHome(t)
	makeSkill(t, home, "alpha", "alpha-skill", "第一个技能")
	makeSkill(t, home, "beta", "beta-skill", "第二个技能")

	list, err := ListSkills()
	if err != nil {
		t.Fatal(err)
	}
	if len(list) != 2 {
		t.Fatalf("应扫到 2 个 Skill，实际 %d", len(list))
	}
	if list[0].Name != "alpha-skill" {
		t.Errorf("应按名称排序，首个为 %q", list[0].Name)
	}
	if list[0].Description != "第一个技能" {
		t.Errorf("描述解析错误: %q", list[0].Description)
	}
	if list[0].Source != "user" {
		t.Errorf("来源应为 user，实际 %q", list[0].Source)
	}
	if !list[0].Enabled {
		t.Error("默认应启用")
	}
}

func TestToggleSkill(t *testing.T) {
	home := withTempHome(t)
	makeSkill(t, home, "alpha", "alpha-skill", "d")

	if err := ToggleSkill("alpha-skill", false); err != nil {
		t.Fatal(err)
	}
	list, _ := ListSkills()
	if list[0].Enabled {
		t.Error("禁用后 Enabled 应为 false")
	}

	// 禁用名单要落盘
	if _, err := os.Stat(filepath.Join(home, ".claude", "disabled-skills.json")); err != nil {
		t.Error("禁用名单未写入文件")
	}

	if err := ToggleSkill("alpha-skill", true); err != nil {
		t.Fatal(err)
	}
	list, _ = ListSkills()
	if !list[0].Enabled {
		t.Error("重新启用后 Enabled 应为 true")
	}
}

func TestParseFrontmatterFallback(t *testing.T) {
	// 没有 frontmatter 时用目录名兜底
	name, desc := parseFrontmatter("# 普通 Markdown\n\n正文", "dir-name")
	if name != "dir-name" {
		t.Errorf("应回退到目录名，实际 %q", name)
	}
	if desc != "" {
		t.Errorf("无 frontmatter 时描述应为空，实际 %q", desc)
	}
}

func TestParseFrontmatterMultilineDegrades(t *testing.T) {
	// 多行写法 `description: >` 退化为空，而不是把 ">" 当描述
	_, desc := parseFrontmatter("---\nname: x\ndescription: >\n  很长的一段\n---\n", "d")
	if desc == ">" || desc == "|" {
		t.Errorf("多行标记不应被当作描述: %q", desc)
	}
}

func TestEmptySkillsDir(t *testing.T) {
	withTempHome(t)
	list, err := ListSkills()
	if err != nil {
		t.Fatal(err)
	}
	if len(list) != 0 {
		t.Errorf("目录不存在时应返回空列表，实际 %d", len(list))
	}
}
