//! 工作区内文本文件的读取（供文件预览）。
//!
//! 预览入口来自模型输出，属于不可信内容：不加限制的话，一个指向
//! ~/.ssh/id_rsa 或 .claude/settings.json 的链接点一下就能把私钥
//! / API Token 明文显示在界面上。因此必须限定在工作区内。

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// 这些扩展名不可能以文本方式有意义地展示。
const BINARY_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "pdf", "zip", "gz", "7z", "rar", "exe",
    "dll", "so", "woff", "woff2", "ttf", "mp3", "mp4", "mov", "dylib", "node", "dmg", "app",
    "pkg", "deb",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub ext: String,
    pub text: String,
    pub size: u64,
}

/// p 是否在 root 内（或等于 root）。Windows 下忽略大小写。
///
/// 必须剥掉 canonicalize 加的 `\?\` 前缀：它只出现在真实路径上，
/// 不剥的话字面路径（无前缀）和真实路径（有前缀）永远比较不相等。
fn within(p: &Path, root: &Path) -> bool {
    let norm = |x: &Path| {
        let s = x.to_string_lossy().to_string();
        let s = s.strip_prefix(r"\?\").unwrap_or(&s).to_string();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    let (p, root) = (norm(p), norm(root));
    p == root
        || p.starts_with(&format!("{root}/"))
        || p.starts_with(&format!("{root}\\"))
}

/// 读取工作区内的文本文件。`roots` 是全部工作区目录。
pub fn read(roots: &[String], path: &str) -> Result<FileContent, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("路径不能为空".into());
    }
    // UNC（\\host\share）会在打开时向远端发起 SMB 认证，可能外泄 NTLM 凭据
    if path.starts_with("\\\\") || path.starts_with("//") {
        return Err("不支持网络路径".into());
    }

    let literal = PathBuf::from(path);

    // 字面路径先查一遍
    let root_paths: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
    if !root_paths.iter().any(|r| within(&literal, r)) {
        return Err("只能预览当前工作区内的文件".into());
    }

    // 再解析真实路径：工作区内可以放一个指向 ~/.claude 的 junction/symlink，
    // 字面路径检查会放行，实际读到的却是工作区外的文件
    let real = fs::canonicalize(&literal).map_err(|_| "文件不存在或无法访问".to_string())?;
    let real_roots: Vec<PathBuf> = root_paths
        .iter()
        .map(|r| fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .collect();
    if !real_roots.iter().any(|r| within(&real, r)) {
        return Err("该路径指向工作区外，已阻止预览".into());
    }

    let info = fs::metadata(&real).map_err(|_| "文件不存在或无法访问".to_string())?;
    if !info.is_file() {
        return Err("该路径不是文件".into());
    }
    if info.len() > MAX_BYTES {
        return Err(format!(
            "文件过大（{} KB），暂不支持预览",
            info.len() / 1024
        ));
    }

    let ext = real
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if BINARY_EXTS.contains(&ext.as_str()) {
        return Err("这是二进制文件，无法以文本方式预览".into());
    }

    let mut data = fs::read(&real).map_err(|_| "读取失败：文件无法访问".to_string())?;
    // UTF-8 BOM
    if data.starts_with(&[0xef, 0xbb, 0xbf]) {
        data.drain(..3);
    }
    let text = String::from_utf8(data).map_err(|_| "文件不是 UTF-8 文本，暂不支持预览".to_string())?;

    let name = real
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    Ok(FileContent {
        name,
        path: real.to_string_lossy().to_string(),
        ext: if ext.is_empty() { String::new() } else { format!(".{ext}") },
        text,
        size: info.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cw-file-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_file_inside_workspace() {
        let d = tmp_dir();
        let f = d.join("a.txt");
        fs::write(&f, "内容").unwrap();
        let got = read(&[d.to_string_lossy().to_string()], &f.to_string_lossy()).unwrap();
        assert_eq!(got.text, "内容");
        assert_eq!(got.name, "a.txt");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_path_outside_workspace() {
        let d = tmp_dir();
        let outside = tmp_dir().join("secret.txt");
        fs::write(&outside, "私钥").unwrap();
        let err = read(&[d.to_string_lossy().to_string()], &outside.to_string_lossy());
        assert!(err.is_err(), "工作区外文件必须拒绝");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_binary_ext() {
        let d = tmp_dir();
        let f = d.join("a.png");
        fs::write(&f, "x").unwrap();
        let err = read(&[d.to_string_lossy().to_string()], &f.to_string_lossy());
        assert!(err.unwrap_err().contains("二进制"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_non_utf8() {
        let d = tmp_dir();
        let f = d.join("gbk.txt");
        fs::write(&f, [0xd6, 0xd0, 0xce, 0xc4]).unwrap();
        let err = read(&[d.to_string_lossy().to_string()], &f.to_string_lossy());
        assert!(err.unwrap_err().contains("UTF-8"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_unc_path() {
        let err = read(&[], "\\\\server\\share\\a.txt");
        assert!(err.unwrap_err().contains("网络路径"));
    }

    #[test]
    fn strips_utf8_bom() {
        let d = tmp_dir();
        let f = d.join("bom.txt");
        fs::write(&f, [0xef, 0xbb, 0xbf, b'h', b'i']).unwrap();
        let got = read(&[d.to_string_lossy().to_string()], &f.to_string_lossy()).unwrap();
        assert_eq!(got.text, "hi");
        let _ = fs::remove_dir_all(&d);
    }
}
