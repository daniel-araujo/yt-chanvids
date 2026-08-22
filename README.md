# yt-chanvids

Lists all public videos of a YouTube channel, and looks up details about a
channel or a single video, from a channel/video id, username or pasted
URL as plain text or JSON.

```
$ yt-chanvids list PewDiePie
https://www.youtube.com/watch?v=0zYI8FjSF_k
https://www.youtube.com/watch?v=X4dAPKYPhDQ
https://www.youtube.com/watch?v=LZ0rGTsdfwk
[...]
```


## Installation

### Crates.io

This program is published on Crates.io so you can easily get it by running cargo
install, like so:

```
cargo install yt-chanvids
```

All binaries installed with cargo install are stored in the installation root’s
bin folder. If you installed Rust using rustup.rs and don’t have any custom
configurations, this directory will be $HOME/.cargo/bin. Ensure that directory
is in your $PATH to be able to run programs you’ve installed with cargo install.

### Arch Linux

You can find it in the AUR

```
https://aur.archlinux.org/packages/yt-chanvids/
```


## Usage

```
yt-chanvids [OPTIONS] [--] CHANNEL-ID|USERNAME
yt-chanvids list [OPTIONS] [--] CHANNEL-ID|USERNAME
yt-chanvids channel [OPTIONS] [--] CHANNEL-ID|USERNAME
yt-chanvids video [OPTIONS] [--] VIDEO-ID
```

You can pass a username or a channel id, or paste a channel URL directly
(a `/channel/...`, `/c/...`, `/user/...` or `/@handle` link) and the id or
handle will be extracted from it. `VIDEO-ID` similarly accepts a pasted
video URL (a `watch`, `youtu.be`, `shorts`, `embed` or `live` link).

The bare form is a backwards-compatible alias for `list`. Both produce a
line for each video that they can find. A line only contains a URL to the
video.

The `channel` subcommand prints a channel's title, thumbnail and description.
The `video` subcommand prints a video's title, publish date, duration (in
seconds) and description.

Passing `--json` to the `list`, `channel` or `video` subcommands prints
their output as JSON instead of plain text.

The exit code will be 0 if the program does not encounter any problems. Any
other value indicates a problem and you will most likely find an error message
in the standard error stream.

The `--` sequence is useful for preventing channel ids and usernames to be
interpreted as options if they begin with dashes.


## Examples

The following examples work on bash. Long output is redacted with
`[...]`.

You can get a list by either passing a channel id:

```
# Passing a channel id.
$ yt-chanvids list UCR4s1DE9J4DHzZYXMltSMAg
https://www.youtube.com/watch?v=yAmGdn9t5Rs
https://www.youtube.com/watch?v=_w6-iHbtn-Y
https://www.youtube.com/watch?v=K1x2Nox-f1A
https://www.youtube.com/watch?v=gFm3brOdxcw
https://www.youtube.com/watch?v=d63CSqjM44k
[...]
```

Or a username:

```
# Passing a username.
$ yt-chanvids list HowToBasic
https://www.youtube.com/watch?v=yAmGdn9t5Rs
https://www.youtube.com/watch?v=_w6-iHbtn-Y
https://www.youtube.com/watch?v=K1x2Nox-f1A
https://www.youtube.com/watch?v=gFm3brOdxcw
https://www.youtube.com/watch?v=d63CSqjM44k
[...]
```

With redirection you're able to save the list to a file:

```
# Saves list to a file.
$ yt-chanvids list Vsauce > to-watch.txt

# Appends more links to the existing file.
$ yt-chanvids list Vsauce2 >> to-watch.txt
```

Pipe the list to another command:

```
# Counts the number of public videos of a channel.
$ yt-chanvids list PewDiePie | wc -l
2929
```

And even perform complex operations:

```
# Shortens urls.
$ yt-chanvids list PewDiePie | sed "s/^https:\/\/www\.youtube\.com\/watch?v=/https:\/\/youtu.be\//"
https://youtu.be/0zYI8FjSF_k
https://youtu.be/X4dAPKYPhDQ
https://youtu.be/LZ0rGTsdfwk
[...]
```


Getting details about a channel:

```
$ yt-chanvids channel PewDiePie
Title: PewDiePie
Thumbnail: https://yt3.googleusercontent.com/[...]
Description: I make videos.
```

Or a video:

```
$ yt-chanvids video jNQXAC9IVRw
Title: Me at the zoo
Published: 2005-04-23T20:31:52-07:00
Duration: 19s
Description: [...]
```

You can also paste a full URL instead of an id or username:

```
$ yt-chanvids video https://youtu.be/jNQXAC9IVRw
Title: Me at the zoo
Published: 2005-04-23T20:31:52-07:00
Duration: 19s
Description: [...]
```

Add `--json` to `list`, `channel` or `video` for JSON output instead:

```
$ yt-chanvids channel --json PewDiePie
{
  "description": "I make videos.",
  "thumbnail": "https://yt3.googleusercontent.com/[...]",
  "title": "PewDiePie"
}
```


## Contributing

The easiest way to contribute is by starring this project on GitHub!

https://github.com/daniel-araujo/yt-chanvids

If you've found a bug, would like to suggest a feature or need help, feel free
to create an issue on GitHub:

https://github.com/daniel-araujo/yt-chanvids/issues


## License

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

[Rust]: https://www.rust-lang.org
[Cargo]: https://crates.io
