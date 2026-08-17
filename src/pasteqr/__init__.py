"""pasteqr —— 剪贴板二维码一键开链接的小工具。

运行后出现 `paste> ` 提示符，把二维码图片复制进剪贴板后按回车
（或者直接粘贴一个图片文件路径），识别出链接后自动用系统默认
浏览器打开；一张图里有多个二维码时列出序号供选择。
"""

from __future__ import annotations

import os
import io
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image
import zxingcpp


def clipboard_image_bytes() -> bytes | None:
    """从系统剪贴板读图片二进制，优先 Wayland 的 wl-paste，兜底 xclip。"""
    if os.environ.get("WAYLAND_DISPLAY") and shutil.which("wl-paste"):
        for mime in ("image/png", "image/jpeg", "image/webp"):
            p = subprocess.run(
                ["wl-paste", "--type", mime], capture_output=True
            )
            if p.returncode == 0 and p.stdout:
                return p.stdout
    if shutil.which("xclip"):
        for mime in ("image/png", "image/jpeg"):
            p = subprocess.run(
                ["xclip", "-selection", "clipboard", "-t", mime, "-o"],
                capture_output=True,
            )
            if p.returncode == 0 and p.stdout:
                return p.stdout
    return None


def clipboard_text() -> str | None:
    """读剪贴板纯文本，用于判断是不是粘了个文件路径。"""
    if os.environ.get("WAYLAND_DISPLAY") and shutil.which("wl-paste"):
        p = subprocess.run(["wl-paste", "--no-newline"], capture_output=True)
        if p.returncode == 0 and p.stdout:
            return p.stdout.decode("utf-8", "replace").strip()
    if shutil.which("xclip"):
        p = subprocess.run(
            ["xclip", "-selection", "clipboard", "-o"], capture_output=True
        )
        if p.returncode == 0 and p.stdout:
            return p.stdout.decode("utf-8", "replace").strip()
    return None


def decode_qr(data: bytes) -> list[str]:
    """解码图片字节里的所有二维码，返回去重后的链接列表。"""
    try:
        img = Image.open(io.BytesIO(data))
        img.load()
    except Exception:
        return []
    if img.mode not in ("L", "RGB"):
        img = img.convert("RGB")

    seen: set[str] = set()
    results: list[str] = []

    def collect(im: Image.Image) -> None:
        try:
            found = zxingcpp.read_barcodes(
                im, formats=zxingcpp.BarcodeFormat.QRCode
            )
        except Exception:
            return
        for b in found:
            t = b.text
            if t and t not in seen:
                seen.add(t)
                results.append(t)

    collect(img)
    # 小图或没解出来时，2x 放大再试（NEAREST 保持二维码像素锐利）
    if not results and img.size[0] * img.size[1] < 4_000_000:
        w, h = img.size
        collect(img.resize((w * 2, h * 2), Image.NEAREST))
    return results


def open_url(url: str) -> bool:
    """用系统默认方式打开链接（后台执行，不阻塞）。"""
    try:
        subprocess.Popen(
            ["xdg-open", url],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return True
    except FileNotFoundError:
        return False


def _read_from_input(resp: str) -> bytes | None:
    """优先把用户输入当图片文件路径读，否则回退到剪贴板。"""
    if resp:
        path = Path(resp).expanduser()
        if path.is_file():
            try:
                return path.read_bytes()
            except OSError as exc:
                print(f"读文件失败: {exc}", file=sys.stderr)
                return None
    return clipboard_image_bytes()


def main() -> None:
    print("把二维码图片复制到剪贴板后按回车，或直接粘贴图片路径；Ctrl-C 退出", file=sys.stderr)
    while True:
        try:
            resp = input("paste> ").strip()
        except (EOFError, KeyboardInterrupt):
            print(file=sys.stderr)
            return

        data = _read_from_input(resp)
        if data is None:
            # 剪贴板里没有图片，看看是不是粘了文件路径文本
            text = clipboard_text()
            if text:
                path = Path(text).expanduser()
                if text.startswith("file://"):
                    path = Path(text.removeprefix("file://")).expanduser()
                if path.is_file():
                    try:
                        data = path.read_bytes()
                    except OSError as exc:
                        print(f"读文件失败: {exc}", file=sys.stderr)
            if data is None:
                print("剪贴板里没有图片，也没找到文件路径", file=sys.stderr)
                continue

        urls = decode_qr(data)
        if not urls:
            print("没识别到二维码", file=sys.stderr)
            continue

        if len(urls) == 1:
            print(f"打开: {urls[0]}")
            if not open_url(urls[0]):
                print("xdg-open 不可用", file=sys.stderr)
            continue

        for i, u in enumerate(urls, 1):
            print(f"{i}: {u}")
        try:
            sel = input("Selection? > ").strip()
        except (EOFError, KeyboardInterrupt):
            print(file=sys.stderr)
            return
        try:
            idx = int(sel)
        except ValueError:
            print("无效输入，跳过", file=sys.stderr)
            continue
        if 1 <= idx <= len(urls):
            print(f"打开 #{idx}: {urls[idx - 1]}")
            open_url(urls[idx - 1])
        else:
            print("序号超出范围", file=sys.stderr)
