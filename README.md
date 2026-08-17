# pasteqr

剪贴板二维码一键开链接的小工具。

## 用法

```bash
uv run pasteqr
```

运行后出现 `paste> ` 提示符：

1. 把二维码图片复制进剪贴板（截图、浏览器右键复制图片都行），然后按回车；
2. 或者直接把图片文件路径粘贴进来再回车，两种方式都支持。

识别结果：

- 只有一个二维码 → 直接用系统默认浏览器打开链接；
- 有多个二维码 → 一行一个列出序号和链接，输入序号打开对应链接。

按 `Ctrl-C` 退出。

## 依赖

- `uv`（包管理）
- Wayland 下需要 `wl-clipboard`（提供 `wl-paste`）；X11 下需要 `xclip`
- `xdg-utils`（提供 `xdg-open`，一般桌面环境自带）

## 原理

- `wl-paste --type image/png` / `xclip` 从剪贴板读图片
- Pillow 读取图片字节，`zxing-cpp` 解码多个二维码（小图自动 2x 放大兜底）
- `xdg-open` 打开链接
