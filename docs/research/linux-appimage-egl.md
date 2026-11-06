# The Linux AppImage aborting on launch with an EGL error

Issue 52: an AppImage from 0.3.0 aborted on Manjaro with `Could not create surfaceless EGL
display: EGL_BAD_ALLOC. Aborting...`.

## The cause

Older AppImages carried their own `libwayland-client.so.0`, from the Ubuntu 22.04 machine that
built them, and the launcher puts the AppImage's libraries first. The computer's own Mesa then
loaded that old copy, which lacks symbols current Mesa needs. Mesa's EGL driver fails to load,
WebKitGTK can't make an EGL display, and it aborts. Which EGL error it prints varies by machine.
WebKit's rendering settings, such as `WEBKIT_DISABLE_DMABUF_RENDERER`, don't help.

The Tauri bundler stopped including `libwayland-client` with its update of linuxdeploy. The
other `libwayland-*` copies are still bundled and leave no missing symbol.

## Checked (2026-11-06)

In an Arch Linux container (x86_64 under emulation; Mesa 26.2.6, wayland 1.26.0, software
rendering), with each AppImage's contents run through its own `AppRun` for 20 seconds:

| AppImage | `libwayland-client` inside | Missing symbols in Mesa's EGL | X11 | Wayland (headless) |
|---|---|---|---|---|
| Schedule 0.3.0 | yes | 3 | `EGL_BAD_PARAMETER`, abort | GTK fails to start |
| Schedule 0.7.0 | no | 0 | stays running | stays running |

The container shows the same failure with a different EGL error from the report, since it has no
GPU. Not checked on the reporter's hardware, or by a person looking at the window.

## Keeping it fixed

`release.yml` now fails if `libwayland-client` is inside the AppImage.

## For anyone on an older AppImage

Update, install the `.deb`, or use the computer's own library:

    LD_PRELOAD=/usr/lib/libwayland-client.so.0 ./Method-Schedule-linux-x86_64.AppImage

The path is `/usr/lib64/` on some distributions and `/usr/lib/x86_64-linux-gnu/` on Debian-based ones.
