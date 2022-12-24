# needle

[![Crates.io](https://img.shields.io/crates/v/needle-rs)](https://crates.io/crates/needle-rs)
[![docs.rs](https://img.shields.io/docsrs/needle-rs?label=docs.rs)](https://docs.rs/needle-rs/)
[![test](https://github.com/aksiksi/needle/actions/workflows/test.yml/badge.svg)](https://github.com/aksiksi/needle/actions/workflows/test.yml)
[![release](https://github.com/aksiksi/needle/actions/workflows/release-needle.yml/badge.svg)](https://github.com/aksiksi/needle/actions/workflows/publish-release.yml)

A tool that finds a needle (opening/intro and ending/credits) in a haystack (TV or anime episode).

## Demo

[![asciicast](https://asciinema.org/a/dzkiQNdR9FXymBte3cx0WP8yB.svg)](https://asciinema.org/a/dzkiQNdR9FXymBte3cx0WP8yB?autoplay=1)

## Quickstart

Download and run the `needle` Docker container:

```
docker run ghcr.io/aksiksi/needle:latest --help
```

**Note:** If you just want to install the binary, skip over to the [install](#install) section before proceeding.

Run a search for opening and endings in the first three episodes of [Land of the Lustrous](https://en.wikipedia.org/wiki/Land_of_the_Lustrous_(TV_series)):

```
$ needle search --analyze ~/Movies/land-of-lustrous-ep1.mkv ~/Movies/land-of-lustrous-ep2.mkv ~/Movies/land-of-lustrous-ep3.mkv

~/Movies/land-of-lustrous-ep1.mkv

* Opening - N/A
* Ending - "22:10s"-"23:39s"

~/Movies/land-of-lustrous-ep2.mkv

* Opening - "00:43s"-"02:12s"
* Ending - "22:10s"-"23:56s"

~/Movies/land-of-lustrous-ep3.mkv

* Opening - "00:40s"-"02:08s"
* Ending - "22:09s"-"23:56s"
```

Run the same search as above, but write the results to a JSON file stored alongside each video (called a "skip file"):

```
$ needle search --analyze --no-display --write-skip-files ~/Movies/land-of-lustrous-ep1.mkv ~/Movies/land-of-lustrous-ep2.mkv ~/Movies/land-of-lustrous-ep3.mkv

$ cat ~/Movies/land-of-lustrous-ep1.needle.skip.json
{"opening":null,"ending":[1331.6644,1419.0249],"md5":"14bfa97f85d86f74e1ab5a26066f9181"}%
```

## Overview

`needle` has two subcommands: 1) **analyze** and 2) **search**.

You may have noticed that we only used the **search** subcommand in the examples above. You also likely noticed that it takes quite a bit to of time to spit out results. Well, it turns out that decoding and resampling audio streams takes *way* longer than searching for openings and endings.

That's where the **analyze** command comes in. Using this subcommand, you can *pre-compute* the required data and store it alongside video files (just like with skip files). The pre-computed data is stored in a compact binary format and is much smaller in size than the audio stream.

Let's try it out with the same files as above:

```
$ needle analyze ~/Movies/land-of-lustrous-ep1.mkv ~/Movies/land-of-lustrous-ep2.mkv ~/Movies/land-of-lustrous-ep3.mkv

$ ls -la ~/Movies/land-of-lustrous-*.needle.dat
-rw-r--r--  1 aksiksi  staff  76128 Jul  2 20:09 ~/Movies/land-of-lustrous-ep1.needle.dat
-rw-r--r--  1 aksiksi  staff  76128 Jul  2 20:09 ~/Movies/land-of-lustrous-ep2.needle.dat
-rw-r--r--  1 aksiksi  staff  76128 Jul  2 20:09 ~/Movies/land-of-lustrous-ep3.needle.dat
```

The frame hash files are quite small: on the order of 4 KB per minute of audio. Note that the size will change based on how you configure the analyzer.

Once we have these pre-computed files, we can re-run the search step, but this time we can omit the `--analyze` flag:

```
$ needle search ~/Movies/land-of-lustrous-ep1.mkv ~/Movies/land-of-lustrous-ep2.mkv ~/Movies/land-of-lustrous-ep3.mkv

~/Movies/land-of-lustrous-ep1.mkv

* Opening - N/A
* Ending - "22:10s"-"23:39s"

~/Movies/land-of-lustrous-ep2.mkv

* Opening - "00:43s"-"02:12s"
* Ending - "22:10s"-"23:56s"

~/Movies/land-of-lustrous-ep3.mkv

* Opening - "00:40s"-"02:08s"
* Ending - "22:09s"-"23:56s"
```

On my machine (M1 Macbook Pro), the **analyze** step takes 10 seconds, while the **search** using pre-computed data takes less than 1 second.

Let's try running **analyze** and **search** for Season 4 of [Attack on Titan](https://en.wikipedia.org/wiki/Attack_on_Titan_(TV_series)) (yes, you can specify directories!):

```
$ time needle analyze ~/Movies/Season\ 04
needle analyze ~/Movies/Season\ 04 --force  87.37s user 6.51s system 783% cpu 11.983 total

$ time needle search ~/Movies/Season\ 04
needle search ~/Movies/Season\ 04  32.59s user 8.82s system 737% cpu 5.617 total
```

In this case, the search step ran about 2x faster than the analyze step. Note that needle utilizes all available cores by default (via `rayon`), but you can disable this by passing in `--no-threading`. For reference, this directory contains 28 episodes of Attack on Titan (~20 minutes each).

## Configuration

TODO

## Install

There are currently three ways to install `needle`:

1. Run `needle` in a Docker container (`amd64` only!):

```
docker run ghcr.io/aksiksi/needle:latest --help
```

2. Grab the latest binary from the [releases page](https://github.com/aksiksi/needle/releases)
    * **Prerequisites:** Install the [runtime dependencies](#runtime-dependencies) for your platform

3. Build and install the latest release:
    * **Prerequisites:** Install the [build dependencies](#build) for your platform

```
cargo install needle-rs
```

4. Build latest version (HEAD) from source:
    * **Prerequisites:** Install the [build dependencies](#build) for your platform

```
cargo build --release --manifest-path needle/
```

## Runtime Dependencies

### Linux (Debian/Ubuntu)

Download the FFmpeg libraries:

```bash
sudo apt-get install \
    libfftw3-3 \
    libavutil56 \
    libavformat58 \
    libswresample3 \
    libavcodec58
```

### macOS

Install FFmpeg and libraries:

```bash
brew install ffmpeg
```

## Build

### Linux (Debian/Ubuntu)

1. Install `pkg-config`, `cmake`, `libclang`, `fftw3` (optional, but recommended), and the `FFmpeg` libraries:

```bash
sudo apt-get install \
    pkg-config \
    cmake \
    libclang-dev \
    libfftw3-dev \
    libavutil-dev \
    libavformat-dev \
    libswresample-dev \
    libavcodec-dev
```

2. Build:

```bash
cargo install --path .
```

This will **dynamically** link against FFmpeg and statically link `chromaprint`.

#### Dynamic

Install libraries:

```bash
sudo apt-get install \
    pkg-config \
    cmake \
    libclang-dev \
    libfftw3-dev \
    libavutil-dev \
    libavformat-dev \
    libswresample-dev \
    libavcodec-dev
```

Build:

```bash
CHROMAPRINT_SYS_DYNAMIC=1 cargo install --path .
```

### macOS

1. Install `cmake` and `FFmpeg`:

```
brew install cmake pkg-config ffmpeg
```

2. Build:

```
cargo install --path .
```

This will **dynamically** link against FFmpeg. `chromaprint` will be statically linked.

### Windows

1. Install `cargo-vcpkg`:

```
cargo install cargo-vcpkg
```

2. Install `vcpkg` deps:

```
cargo vcpkg build
```

3. Build:

```bash
# Statically link against both FFmpeg and chromaprint
cargo build --release --features static
```

#### Dynamic

1. Set the following environment variables:

    a. To dynamically link **both** FFmpeg and `chromaprint`:

    ```powershell
    # Powershell
    $env:VCPKGRS_DYNAMIC='1'
    $env:VCPKGRS_TRIPLET='x64-windows'
    ```

    ```bash
    # Git bash
    export VCPKGRS_DYNAMIC=1
    export VCPKGRS_TRIPLET='x64-windows'
    ```

    b. Just `chromaprint`:

    ```powershell
    # Powershell
    $env:CHROMAPRINT_SYS_DYNAMIC='1'
    ```

    ```bash
    # Git bash
    export CHROMAPRINT_SYS_DYNAMIC=1
    ```

2. Build deps:

```
cargo vcpkg build
```

3. Build `needle`:

```
cargo build --release
```

## License

This work is dual-licensed under MIT and LGPL 2.1 (or later).

If you choose to **statically** link FFmpeg, this is licensed as LGPL 2.1 (or later) due to FFmpeg. Otherwise, you can use the MIT license.

`SPDX-License-Identifier: MIT OR LGPL-2.1-or-later`
