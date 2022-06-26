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
3. Add `vcpkg` bin directory to path (for DLL lookup): `$VCPKG_ROOT\installed\x64-windows\bin`
4. Build and run: `cargo run`

**Note:** Static linking does not work on Windows due to issues with static linking `ffmpeg` using vcpkg. See: https://github.com/microsoft/vcpkg/issues/9571

