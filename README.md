# showcase-dl
*A parallel downloader to create private backups of embedded vimeo videos and vimeo showcases.*

## Why does this exist?
The need arose to secure a private backup of a some embedded vimeo showcases.

As it turned out, the almighty [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) did not support the creation of local backups from vimeo showcases.

This tool is the result of some initial tinkering to try and automate the extraction of data necessary to download vimeo showcase videos anyway.

This, however, soon turned into perfectionist yak shaving, ending up in a terminal user interface for parallel downloading of all embedded vimeo videos and vimeo showcases on a webpage.

## What does it do and how do I use it?

Currently no prebuild binaries are provided. To compile the tool, simply get `rustup` and `cargo` by following the instructions at [https://rustup.rs/].

Then `git clone` this repository (or download as `.zip` file and extract), and run `cargo build --release` in the project folder. Cargo will make sure to download all dependencies from [https://crates.io], install and compile them; then it will compile the app for you.

The finished executable binary will be found at `<project folder>/target/release/showcase-dl` on Linux or Mac,
or at `<project folder>/target/release/showcase-dl.exe` on Windows.

To start downloads, run the executable in your terminal, passing the target page's URL as only argument.

![Download progress](/img/In%20progress%2C%20spaced.png)

You can close the app at any time by pressing either the `Q` or `Esc` key, or the combination `Ctrl+C`.

As long as you do not close the app ahead of time, your videos will be downloaded concurrently, each in their own time.

![Partially finished](/img/In%20progress%2C%20partially%20finished.png)

After all downloads have finished, the app will remain open. This way, you can just go do other stuff, and come back to a nice status overview. Close the app with the `Q` or `Esc` key, or the combination `Ctrl+C`.

## Credentials
This little tool is standing on the shoulders of giants.

- 🦀 [The Rust programming language](https://www.rust-lang.org/)
- 🗼 [The Tokio async runtime and ecosystem](https://tokio.rs/)
- 📺 [The downloader for everything except vimeo showcases: `yt-dlp`](https://github.com/yt-dlp/yt-dlp)
- 🖥️ [The `tui-rs` terminal user interface library](https://github.com/fdehau/tui-rs)
- 💥 [`color-eyre` and its predecessor `anyhow` for ergonomic error handling](https://github.com/yaahc/color-eyre)
- 💬 [The `clap` commandline argument parsing library](https://github.com/clap-rs/clap)
- 🍵 [Jon Gjengset and his awesome "Crust of Rust" series of videos](https://www.youtube.com/playlist?list=PLqbS7AVVErFiWDOAVrPt7aYmnuuOLYvOa) (You should totally [buy his book](https://nostarch.com/rust-rustaceans)!)

## Disclaimer
This tool has been built to help create legal private backups of your own vimeo videos and showcases.
Make sure you hold the copyright of any material, and tread on safe legal ground according to
the country you live in, before you use this tool!
This tool does not itself download any video material. It merely spawns and sheperds processes of [`yt-dlp`](https://github.com/yt-dlp/yt-dlp).
