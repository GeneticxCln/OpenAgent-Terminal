<p align="center">
    <img width="400" alt="OpenAgent Terminal Logo" src="../extra/logo/openagent-terminal.png" style="max-width: 100%; height: auto;">
</p>

<h1 align="center">OpenAgent Terminal - A fast, cross-platform, OpenGL terminal emulator</h1>

<p align="center">
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml">
    <img alt="CI" src="https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml/badge.svg?branch=main">
  </a>
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/releases">
    <img alt="Latest release" src="https://img.shields.io/github/v/release/GeneticxCln/OpenAgent-Terminal?include_prereleases&sort=semver">
  </a>
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE">
    <img alt="License" src="https://img.shields.io/github/license/GeneticxCln/OpenAgent-Terminal">
  </a>
</p>

## About

OpenAgent Terminal is a modern terminal emulator that comes with sensible defaults, but
allows for extensive [configuration](#configuration). By integrating with other
applications, rather than reimplementing their functionality, it manages to
provide a flexible set of [features](../docs/features.md) with high performance.
The supported platforms currently consist of BSD, Linux, macOS and Windows.

The software is considered to be at a **beta** level of readiness; there are
a few missing features and bugs to be fixed, but it is already used by many as
a daily driver.

Precompiled binaries are available from the [GitHub releases page](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).


## Features

You can find an overview over the features available in OpenAgent Terminal here: [docs/features.md](../docs/features.md)

## Further information

- Releases: https://github.com/GeneticxCln/OpenAgent-Terminal/releases
- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)

## Installation

OpenAgent Terminal can be installed by using various package managers on Linux, BSD,
macOS and Windows.

Prebuilt binaries for macOS and Windows can also be downloaded from the
[GitHub releases page](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).

For everyone else, the detailed instructions to install OpenAgent Terminal can be found
[here](../INSTALL.md).

### Requirements

- At least OpenGL ES 2.0
- [Windows] ConPTY support (Windows 10 version 1809 or higher)

## Configuration

You can find the documentation for OpenAgent Terminal's configuration in `man 5
openagent-terminal`.

OpenAgent Terminal doesn't create the config file for you, but it looks for one in the
following locations:

1. `$XDG_CONFIG_HOME/openagent-terminal/openagent-terminal.toml`
2. `$XDG_CONFIG_HOME/openagent-terminal.toml`
3. `$HOME/.config/openagent-terminal/openagent-terminal.toml`
4. `$HOME/.openagent-terminal.toml`
5. `/etc/openagent-terminal/openagent-terminal.toml`

On Windows, the config file will be looked for in:

* `%APPDATA%\\openagent-terminal\\openagent-terminal.toml`

## Contributing

A guideline about contributing to OpenAgent Terminal can be found in the
[`CONTRIBUTING.md`](CONTRIBUTING.md) file.

## FAQ

**_Is it really the fastest terminal emulator?_**

Benchmarking terminal emulators is complicated. OpenAgent Terminal uses
[vtebench](https://github.com/alacritty/vtebench) to quantify terminal emulator
throughput and manages to consistently score better than the competition using
it. If you have found an example where this is not the case, please report a
bug.

Other aspects like latency or framerate and frame consistency are more difficult
to quantify. Some terminal emulators also intentionally slow down to save
resources, which might be preferred by some users.

If you have doubts about OpenAgent Terminal's performance or usability, the best way to
quantify terminal emulators is always to test them with **your** specific
usecases.

**_Why isn't feature X implemented?_**

OpenAgent Terminal has many great features, but not every feature from every other
terminal. This could be for a number of reasons, but sometimes it's just not a
good fit for OpenAgent Terminal. This means you won't find things like tabs or splits
(which are best left to a window manager or [terminal multiplexer][tmux]) nor
niceties like a GUI config editor.

[tmux]: https://github.com/tmux/tmux

## License

OpenAgent Terminal is released under the [Apache License, Version 2.0].

[Apache License, Version 2.0]: https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE

