//! 工作区内文本文件的读取（供文件预览）。
//!
//! 预览入口来自模型输出，属于不可信内容：不加限制的话，一个指向
//! ~/.ssh/id_rsa 或 .claude/settings.json 的链接点一下就能把私钥
//! / API Token 明文显示在界面上。因此必须限定在工作区内。

use base64::Engine;
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
/// 两个坑都要处理：
/// 1. canonicalize 会加 `\?\` 前缀，不剥的话字面路径与真实路径永远不相等。
/// 2. 斜杠方向必须统一。模型输出里 `C:/a/b.md` 很常见，只认 `\` 会把它判成工作区外。
fn within(p: &Path, root: &Path) -> bool {
    let norm = |x: &Path| {
        // 先统一方向，再剥前缀——前缀本身是 `\\?\`，换过方向就变成 `//?/`
        let s = x.to_string_lossy().replace('\\', "/");
        let s = s.strip_prefix("//?/").unwrap_or(&s).to_string();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    let (p, root) = (norm(p), norm(root));
    p == root || p.strip_prefix(&root).is_some_and(|rest| rest.starts_with('/'))
}

/// base64 编码器。附件图片要转成文本才能进 NDJSON。
const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

/// 图片内容：data URL 供 <img> 预览，纯 base64 供发给模型。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    pub name: String,
    pub media_type: String,
    pub data: String,
    pub preview: String,
    pub size: u64,
}

/// 读本地图片。用户主动选的附件不受工作区限制——那是他明确指定要发的文件。
pub fn read_image(path: &str) -> Result<ImageData, String> {
    let p = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err("路径不能为空".into());
    }
    let info = fs::metadata(&p).map_err(|_| "图片不存在或无法访问".to_string())?;
    if !info.is_file() {
        return Err("该路径不是文件".into());
    }

    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let media_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return Err("不是支持的图片格式（png/jpg/gif/webp/bmp）".into()),
    };

    let data = fs::read(&p).map_err(|_| "读取图片失败".to_string())?;
    let b64 = BASE64.encode(&data);
    Ok(ImageData {
        name: p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        media_type: media_type.into(),
        preview: format!("data:{media_type};base64,{b64}"),
        data: b64,
        size: info.len(),
    })
}

/// 批量判断路径是否落在任一工作区目录内。
///
/// 界面用它决定文件卡片可点还是灰显。放后端是因为只有这里能做 canonicalize——
/// 前端手上的工作区路径可能已被删除或移动，按字面比较会误判。
pub fn check(roots: &[String], paths: &[String]) -> Vec<bool> {
    let root_paths: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
    paths
        .iter()
        .map(|p| {
            let literal = PathBuf::from(p.trim());
            if !root_paths.iter().any(|r| within(&literal, r)) {
                return false;
            }
            // 字面通过还不够：工作区内的符号链接可能指向区外
            match fs::canonicalize(&literal) {
                Ok(real) => root_paths
                    .iter()
                    .any(|r| within(&real, &fs::canonicalize(r).unwrap_or_else(|_| r.clone()))),
                Err(_) => false,
            }
        })
        .collect()
}


/// 目录树的一个节点。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    /// true = 目录（可展开），false = 文件
    pub is_dir: bool,
    /// 文件字节数；目录为 None（避免每层都去算总大小）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// 默认不展开的目录。
///
/// 这些目录动辄几万个文件，一次列出来会让界面卡死、也会把用户想找的东西淹掉。
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "target",
    "build",
    ".next",
    ".nuxt",
    "__pycache__",
    ".ruff_cache",
    ".pytest_cache",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
];

/// 列一层目录。
///
/// 只列一层而不是递归整棵树：大项目一次全扫会卡住界面，
/// 展开子目录时再按需调用。
pub fn list_dir(root: &str, rel: &str, show_ignored: bool) -> Result<Vec<TreeNode>, String> {
    let base = PathBuf::from(root);
    if !base.is_dir() {
        return Err(format!("目录不存在: {root}"));
    }

    // rel 是相对路径。必须挡住 `..`，否则可以读工作区外的东西。
    let target = if rel.is_empty() {
        base.clone()
    } else {
        let joined = base.join(rel);
        // 字面检查 + 真实路径检查，防止用 `..` 或符号链接跳出去
        if !within(&joined, &base) {
            return Err("只能浏览工作区内的目录".into());
        }
        let real = fs::canonicalize(&joined).map_err(|_| "目录不存在或无法访问".to_string())?;
        let real_base = fs::canonicalize(&base).unwrap_or_else(|_| base.clone());
        if !within(&real, &real_base) {
            return Err("该路径指向工作区外，已阻止访问".into());
        }
        real
    };

    let rd = fs::read_dir(&target).map_err(|e| format!("读取目录失败: {e}"))?;
    let mut dirs: Vec<TreeNode> = vec![];
    let mut files: Vec<TreeNode> = vec![];

    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        // 隐藏文件默认不显示，但保留了「显示忽略项」开关
        if name.starts_with('.') && !show_ignored && name != ".env" {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();

        if ft.is_dir() {
            if !show_ignored && IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            dirs.push(TreeNode {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir: true,
                size: None,
            });
        } else if ft.is_file() {
            let size = e.metadata().ok().map(|m| m.len());
            files.push(TreeNode {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir: false,
                size,
            });
        }
    }

    // 目录在前，各自按名字排；用不区分大小写的比较，Windows 上更符合直觉
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    Ok(dirs)
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
    fn within_accepts_both_slash_styles() {
        let root = Path::new("/ws/proj");
        assert!(within(Path::new("/ws/proj/a.md"), root), "反斜杠/Unix 形式");
        assert!(within(Path::new("/ws/proj/sub/a.md"), root), "子目录");
        assert!(within(Path::new("/ws/proj"), root), "根本身");
        assert!(!within(Path::new("/ws/proj-other/a.md"), root), "同前缀的兄弟目录不算在内");
        assert!(!within(Path::new("/ws/a.md"), root), "上级目录不算");
    }

    #[test]
    fn read_image_encodes_png() {
        let d = tmp_dir();
        let f = d.join("a.png");
        // 1x1 红色 PNG
        fs::write(
            &f,
            [
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
                0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02,
                0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44,
                0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01,
                0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
                0xae, 0x42, 0x60, 0x82,
            ],
        )
        .unwrap();

        let got = read_image(&f.to_string_lossy()).unwrap();
        assert_eq!(got.media_type, "image/png");
        assert!(got.preview.starts_with("data:image/png;base64,"));
        assert!(!got.data.is_empty());
        assert_eq!(got.name, "a.png");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn read_image_rejects_non_image() {
        let d = tmp_dir();
        let f = d.join("a.txt");
        fs::write(&f, "hello").unwrap();
        assert!(read_image(&f.to_string_lossy()).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn list_dir_hides_ignored_by_default() {
        let d = tmp_dir();
        fs::create_dir_all(d.join("node_modules")).unwrap();
        fs::create_dir_all(d.join("src")).unwrap();
        fs::write(d.join("src/main.ts"), "x").unwrap();
        fs::write(d.join("a.md"), "x").unwrap();

        let root = d.to_string_lossy().to_string();
        let shown = list_dir(&root, "", false).unwrap();
        let names: Vec<&str> = shown.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"src"), "普通目录要显示");
        assert!(names.contains(&"a.md"), "文件要显示");
        assert!(!names.contains(&"node_modules"), "默认应隐藏 node_modules");

        let all = list_dir(&root, "", true).unwrap();
        assert!(
            all.iter().any(|n| n.name == "node_modules"),
            "开了开关就该显示"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn list_dir_puts_dirs_before_files() {
        let d = tmp_dir();
        fs::create_dir_all(d.join("zdir")).unwrap();
        fs::write(d.join("afile.txt"), "x").unwrap();

        let root = d.to_string_lossy().to_string();
        let got = list_dir(&root, "", false).unwrap();
        assert!(got[0].is_dir, "目录应排在文件前");
        assert!(!got[1].is_dir);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn list_dir_rejects_parent_escape() {
        let d = tmp_dir();
        let root = d.to_string_lossy().to_string();
        // `..` 不能跳出工作区
        assert!(list_dir(&root, "..", false).is_err(), "应阻止向上跳出");
        assert!(list_dir(&root, "../..", false).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn list_dir_reports_file_size() {
        let d = tmp_dir();
        fs::write(d.join("a.txt"), "12345").unwrap();
        let root = d.to_string_lossy().to_string();
        let got = list_dir(&root, "", false).unwrap();
        let f = got.iter().find(|n| n.name == "a.txt").unwrap();
        assert_eq!(f.size, Some(5));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn check_flags_inside_and_outside() {
        let d = tmp_dir();
        let inside = d.join("a.txt");
        fs::write(&inside, "x").unwrap();
        let outside = std::env::temp_dir().join("cw-outside-probe.txt");
        fs::write(&outside, "x").unwrap();

        let roots = vec![d.to_string_lossy().to_string()];
        let got = check(
            &roots,
            &[
                inside.to_string_lossy().to_string(),
                outside.to_string_lossy().to_string(),
                d.join("nope.txt").to_string_lossy().to_string(),
            ],
        );
        assert_eq!(got, vec![true, false, false], "区内 true / 区外 false / 不存在 false");

        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn within_windows_normalizes_slashes_and_case() {
        let root = Path::new(r"C:\Users\Me\proj");
        assert!(within(Path::new(r"C:\Users\Me\proj\a.md"), root), "标准反斜杠");
        assert!(within(Path::new("C:/Users/Me/proj/a.md"), root), "正斜杠必须放行");
        assert!(within(Path::new("C:/Users/Me/proj/sub/a.md"), root), "正斜杠子目录");
        assert!(within(Path::new(r"c:\users\me\proj\a.md"), root), "大小写不一致");
        assert!(within(Path::new("C:/Users/Me/proj"), root), "根的正斜杠形式");
        assert!(!within(Path::new("C:/Users/Me/proj2/a.md"), root), "兄弟目录");
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
