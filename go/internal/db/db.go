// Package db 管理 SQLite 连接与表结构校验。
//
// 三份实现共用一个 workbench.db。本包**不主动迁移**：
// 若库版本低于当前实现要求的版本，说明它是由旧版 Node 创建的，
// 提示用户先用 Node 版启动一次完成迁移。这样避免两个实现
// 各自跑迁移、把对方的数据搞坏。
package db

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

// SchemaVersion 本实现期望的库版本，与 Node 版保持一致。
const SchemaVersion = 4

var handle *sql.DB

// Path 返回数据库文件路径（与 Node 版同一位置）。
func Path() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", fmt.Errorf("无法定位用户配置目录: %w", err)
	}
	return filepath.Join(dir, "claude-workbench", "workbench.db"), nil
}

// Open 打开数据库并做完整性校验与连接参数设置。
func Open() (*sql.DB, error) {
	p, err := Path()
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		return nil, fmt.Errorf("创建数据目录失败: %w", err)
	}

	if _, statErr := os.Stat(p); os.IsNotExist(statErr) {
		return nil, fmt.Errorf("数据库不存在：%s\n请先用 Node 版启动一次以创建数据库", p)
	}

	d, err := sql.Open("sqlite", p)
	if err != nil {
		return nil, fmt.Errorf("打开数据库失败: %w", err)
	}

	// 单连接：node:sqlite 是同步的，Go 这边也贴着单连接避免写锁争抢
	d.SetMaxOpenConns(1)

	for _, pragma := range []string{
		"PRAGMA journal_mode = WAL",
		"PRAGMA foreign_keys = ON",
		"PRAGMA busy_timeout = 5000",
	} {
		if _, err := d.Exec(pragma); err != nil {
			d.Close()
			return nil, fmt.Errorf("设置 %s 失败: %w", pragma, err)
		}
	}

	var integrity string
	if err := d.QueryRow("PRAGMA integrity_check").Scan(&integrity); err != nil {
		d.Close()
		return nil, fmt.Errorf("完整性检查失败: %w", err)
	}
	if integrity != "ok" {
		d.Close()
		return nil, fmt.Errorf("数据库损坏（integrity_check = %s）", integrity)
	}

	ver, err := schemaVersion(d)
	if err != nil {
		d.Close()
		return nil, err
	}
	if ver < SchemaVersion {
		d.Close()
		return nil, fmt.Errorf(
			"数据库版本为 %d，本实现需要 %d。\n请先用 Node 版启动一次完成迁移，再用本版本打开",
			ver, SchemaVersion)
	}

	handle = d
	return d, nil
}

func schemaVersion(d *sql.DB) (int, error) {
	var raw string
	err := d.QueryRow("SELECT value FROM meta WHERE key = 'schema_version'").Scan(&raw)
	if err == sql.ErrNoRows {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("读取 schema_version 失败: %w", err)
	}
	var v int
	if _, err := fmt.Sscanf(raw, "%d", &v); err != nil {
		return 0, fmt.Errorf("schema_version 不是数字: %q", raw)
	}
	return v, nil
}

// Get 返回已打开的连接。
func Get() *sql.DB { return handle }

// Close 关闭连接。
func Close() {
	if handle != nil {
		_ = handle.Close()
		handle = nil
	}
}

// Now 返回与 Node 版一致的时间格式（ISO 8601 带毫秒）。
func Now() string {
	return time.Now().UTC().Format("2006-01-02T15:04:05.000Z")
}

// SetForTest 把连接指向给定句柄，仅供单元测试使用。
func SetForTest(d *sql.DB) { handle = d }
