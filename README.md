# needle

A tool that finds a needle (opening/intro and ending/credits) in a haystack (TV or anime episode).

## Build

### Linux

Install the `ffmpeg` dev libraries:

```
sudo apt install \
    libavutil-dev \
    libavformat-dev \
    libswresample-dev \
    libavcodec-dev \
    libavfilter-dev \
    libavdevice-dev
```

### macOS

Install `ffmpeg`:

```
brew install ffmpeg
```

### Windows

1. Install `cargo-vcpkg`: `cargo install cargo-vcpkg`
2. Install `vcpkg` deps: `cargo vcpkg build`
3. Add additional libs to rustcflags:

```toml
# ~/.cargo/config.toml

[target.x86_64-pc-windows-msvc]
# ffmpeg static build
rustflags = [
    "-C", "link-arg=strmiids.lib",
    "-C", "link-arg=mf.lib",
    "-C", "link-arg=mfplat.lib",
    "-C", "link-arg=mfplay.lib",
    "-C", "link-arg=mfreadwrite.lib",
    "-C", "link-arg=mfuuid.lib",
    "-C", "link-arg=dxva2.lib",
    "-C", "link-arg=evr.lib",
    "-C", "link-arg=vfw32.lib",
    "-C", "link-arg=shlwapi.lib",
    "-C", "link-arg=oleaut32.lib"
]
```

4. Build: `cargo build --features static`

**Note:** The steps above assume a *static* build.

