# yt-chanvids

Generates a list of links to all public videos of a YouTube channel.

```
$ yt-chanvids PewDiePie
https://www.youtube.com/watch?v=0zYI8FjSF_k
https://www.youtube.com/watch?v=X4dAPKYPhDQ
https://www.youtube.com/watch?v=LZ0rGTsdfwk
[...]
```


## Installation

This program is published on Crates.io so you can easily get it by running cargo
install, like so:

```
cargo install yt-chanvids
```

All binaries installed with cargo install are stored in the installation root’s
bin folder. If you installed Rust using rustup.rs and don’t have any custom
configurations, this directory will be $HOME/.cargo/bin. Ensure that directory
is in your $PATH to be able to run programs you’ve installed with cargo install.


## Usage

```
yt-chanvids [OPTIONS] [--] CHANNEL-ID|USERNAME
```

You can pass a username or a channel id. They can be found easily in the URL of
a YouTube channel's page.

The program will produce a line for each video that it can find. A line only contains a URL to the video.

The exit code will be 0 if the program does not encounter any problems. Any
other value indicates a problem and you will most likely find an error message
in the standard error stream.

The only option available at the moment is the help option (`-h`, `--help`)
which makes the program do nothing but print usage instructions.

The `--` sequence is useful for preventing channel ids and usernames to be
interpreted as options if they begin with dashes.


## Examples

The following examples work on bash. Long output is redacted with
`[...]`.

You can get a list by either passing a channel id:

```
# Passing a channel id.
$ yt-chanvids UCR4s1DE9J4DHzZYXMltSMAg
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
$ yt-chanvids HowToBasic
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
$ yt-chanvids Vsauce > to-watch.txt

# Appends more links to the existing file.
$ yt-chanvids Vsauce2 >> to-watch.txt
```

Pipe the list to another command:

```
# Counts the number of public videos of a channel.
$ yt-chanvids PewDiePie | wc -l
2929
```

And even perform complex operations:

```
# Shortens urls.
$ yt-chanvids PewDiePie | sed "s/^https:\/\/www\.youtube\.com\/watch?v=/https:\/\/youtu.be\//"
https://youtu.be/0zYI8FjSF_k
https://youtu.be/X4dAPKYPhDQ
https://youtu.be/LZ0rGTsdfwk
[...]
```


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
