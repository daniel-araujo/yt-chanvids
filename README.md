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

At the moment it's only available from source. It should be compilable on any
platform where [Rust] is supported. [Cargo] will do most of the work for you.

```
cargo install --path  .
```

This will compile and install the program to `~/.cargo/bin`. For convenience
you can add that directory to your `PATH` environment variable. The examples
in this file assume that has been done.


## Usage

```
yt-chanvids [OPTIONS] [--] CHANNEL-ID|USERNAME
```

You can pass either the username or the channel id of a YouTube channel (which
can be spotted in the address of their page) and the program will print a line
separated list of links to the standard output stream.

When the program faces an unexpected situation, it will write any diagnostic
message to the standard error stream. It may terminate execution and when it
does it will always return an error exit code.

At the moment there is only the help option (`-h`, `--help`) available which
makes the program do nothing but print usage instructions.  The `--` sequence
is useful to prevent any arguments after it to be interpreted as options.


## Examples

The following examples assume bash as the shell. Long output is redacted with
`[...]`.

Getting the links is as simple as passing a channel id or username to the
program.

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

If your shell supports redirection of output, you can probably do things like
inserting content to files or sending it to the input stream of an other
command. Having the links separated by new lines makes it easy for other
commands to manipulate them.

```
# Creates a file with the list of links.
$ yt-chanvids Vsauce > to-watch.txt
# Inserts more links to the end of the file.
$ yt-chanvids Vsauce2 >> to-watch.txt
```

```
# Counts the number of public videos of a channel.
$ yt-chanvids PewDiePie | wc -l
2929
```

```
# Shortens the links.
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
