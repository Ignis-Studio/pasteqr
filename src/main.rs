use std::collections::HashSet;
use std::env;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use rxing::helpers::detect_multiple_in_luma_with_hints;
use rxing::{BarcodeFormat, DecodeHints};

fn clipboard_image_bytes() -> Option<Vec<u8>> {
    // Wayland: wl-paste --type image/<mime>
    if env::var("WAYLAND_DISPLAY").is_ok() {
        for mime in ["image/png", "image/jpeg", "image/webp"] {
            if let Ok(out) = Command::new("wl-paste")
                .args(["--type", mime])
                .output()
            {
                if out.status.success() && !out.stdout.is_empty() {
                    return Some(out.stdout);
                }
            }
        }
    }
    // X11 兜底
    if let Ok(out) = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-o"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Some(out.stdout);
        }
    }
    None
}

fn clipboard_text() -> Option<String> {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(out) = Command::new("wl-paste").arg("--no-newline").output() {
            if out.status.success() && !out.stdout.is_empty() {
                return Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
        }
    }
    if let Ok(out) = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
    {
        if out.status.success() && !out.stdout.is_empty() {
            return Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
        }
    }
    None
}

fn decode_qr(data: &[u8]) -> Vec<String> {
    let img = match image::load_from_memory(data) {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    let mut hints = DecodeHints::default();
    hints.TryHarder = Some(true);
    hints.PossibleFormats = Some(
        std::collections::HashSet::from([BarcodeFormat::QR_CODE]),
    );

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();

    let collect = |results: Vec<rxing::RXingResult>, seen: &mut HashSet<String>, out: &mut Vec<String>| {
        for r in results {
            let t = r.getText().to_string();
            if !t.is_empty() && seen.insert(t.clone()) {
                out.push(t);
            }
        }
    };

    let results = detect_multiple_in_luma_with_hints(gray.clone().into_raw(), w, h, &mut hints)
        .unwrap_or_default();
    collect(results, &mut seen, &mut out);

    // 小图或没解出来时，2x 放大再试（NEAREST 保持二维码像素锐利）
    if out.is_empty() && w * h < 4_000_000 {
        let big = image::imageops::resize(&gray, w * 2, h * 2, image::imageops::FilterType::Nearest);
        let (bw, bh) = big.dimensions();
        let results =
            detect_multiple_in_luma_with_hints(big.into_raw(), bw, bh, &mut hints).unwrap_or_default();
        collect(results, &mut seen, &mut out);
    }

    out
}

fn open_url(url: &str) -> bool {
    Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

fn read_line(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    let n = io::stdin().lock().read_line(&mut line)?;
    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
    }
    Ok(line.trim().to_string())
}

fn read_image_file(resp: &str) -> Option<Vec<u8>> {
    let p = Path::new(resp).expanduser_if_needed();
    if p.is_file() {
        std::fs::read(p).ok()
    } else {
        None
    }
}

trait ExpandUser {
    fn expanduser_if_needed(&self) -> std::path::PathBuf;
}

impl ExpandUser for Path {
    fn expanduser_if_needed(&self) -> std::path::PathBuf {
        let s = self.to_string_lossy();
        if let Some(rest) = s.strip_prefix("~/") {
            if let Some(home) = env::var_os("HOME") {
                return Path::new(&home).join(rest);
            }
        }
        self.to_path_buf()
    }
}

fn main() {
    eprintln!("把二维码图片复制到剪贴板后按回车，或直接粘贴图片路径；Ctrl-C 退出");
    loop {
        let resp = match read_line("paste> ") {
            Ok(s) => s,
            Err(_) => {
                eprintln!();
                return;
            }
        };

        // 优先把用户输入当图片文件路径读
        let mut data = if !resp.is_empty() {
            read_image_file(&resp)
        } else {
            None
        };

        // 回退到剪贴板图片
        if data.is_none() {
            data = clipboard_image_bytes();
        }

        // 再兜底：剪贴板文本可能是文件路径
        if data.is_none() {
            if let Some(text) = clipboard_text() {
                let path_str = text.strip_prefix("file://").unwrap_or(&text);
                let p = Path::new(path_str).expanduser_if_needed();
                if p.is_file() {
                    data = std::fs::read(p).ok();
                }
            }
        }

        let data = match data {
            Some(d) => d,
            None => {
                eprintln!("剪贴板里没有图片，也没找到文件路径");
                continue;
            }
        };

        let urls = decode_qr(&data);
        if urls.is_empty() {
            eprintln!("没识别到二维码");
            continue;
        }

        if urls.len() == 1 {
            println!("打开: {}", urls[0]);
            if !open_url(&urls[0]) {
                eprintln!("xdg-open 不可用");
            }
            continue;
        }

        for (i, u) in urls.iter().enumerate() {
            println!("{}: {}", i + 1, u);
        }
        let sel = match read_line("Selection? > ") {
            Ok(s) => s,
            Err(_) => {
                eprintln!();
                return;
            }
        };
        match sel.parse::<usize>() {
            Ok(idx) if idx >= 1 && idx <= urls.len() => {
                println!("打开 #{}: {}", idx, urls[idx - 1]);
                open_url(&urls[idx - 1]);
            }
            _ => eprintln!("无效输入，跳过"),
        }
    }
}
