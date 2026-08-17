# pasteqr

Paste and scan it.

## Usage

```bash
uv run pasteqr
```

1. Copy the QR code image or its path, press Enter.
2. Paste the image.
3. Paste the QR code's path and press Enter.

`Ctrl-C` to exit.

## Requirements

- `wl-clipboard` under Wayland or `xclip` under X11.
- `xdg-utils`(Mostly contained by your Distro or DE)

Under the Apache 2.0 License.

By: Zhengyuan Huang <neclyon@qq.com>
