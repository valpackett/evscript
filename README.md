In the X11 world, we had wonderful programs like [xcape] and [xdotool] that injected fake keypresses to do whatever cool things we wanted.
In the brave new world of Wayland, security is king, so there's no access to global input.
No keyloggers, no geeky keyboard tricks, no text macro expanders, no UI testing/automation, screw you.

Wayland compositor authors could have agreed on a protocol like [this one](https://gist.github.com/myfreeweb/7c656d535ae1c5a1336f29d2c1473726) to allow all this useful functionality, but with secure access control, just like on macOS where this requires ticking a checkbox in accessibility settings.
But no, their attitude has been "screw you, because security".
Oh well.

Turns out we can do it low level style! :P
As in, on the evdev level.

There's been some prior art already (e.g. [evdevremapkeys]), but this is way more flexible.

Instead of doing specific simple things…

this is a scripting environment!

[xcape]: https://github.com/alols/xcape
[xdotool]: https://github.com/jordansissel/xdotool
[evdevremapkeys]: https://github.com/philipl/evdevremapkeys

# evscript

A tiny sandboxed [Dyon] scripting environment for evdev input devices.

You get a set of devices.
You can get events from them, emit events into a virtual device (uinput) and print to stdout.
That's it.

[Dyon]: https://github.com/PistonDevelopers/dyon

## Installation

Something like that (with [cargo]):

```bash
git clone https://github.com/myfreeweb/evscript
cd evscript
cargo build --release
install -Ss -o root -m 4755 target/release/evscript /usr/local/bin/evscript
```

evscript is designed for setuid root: right after opening the devices (before executing the script) it drops privileges, chroots and (on FreeBSD) sandboxes itself with Capsicum.

You can allow yourself access to `/dev/input/*` and `/dev/uinput` instead of setuid, but that would allow any random program running as you to work with inputs.

[cargo]: http://doc.crates.io/index.html

## Usage

A simple script looks like this:

```dyon
fn main() ~ evdevs, uinput {
    should_esc := false
    loop {
        evts := next_events(evdevs)
        for i len(evts) {
            evt := evts[i]
            xcape(mut should_esc, evt, KEY_CAPSLOCK(), KEY_ESC())
        }
    }
}
```

Check out the source of the standard library in [src/stdlib.dyon](https://github.com/myfreeweb/evscript/blob/master/src/stdlib.dyon) to see how `xcape` is implemented!

## Contributing

By participating in this project you agree to follow the [Contributor Code of Conduct](https://www.contributor-covenant.org/version/1/4/).

[The list of contributors is available on GitHub](https://github.com/myfreeweb/evscript/graphs/contributors).

## License

This is free and unencumbered software released into the public domain.  
For more information, please refer to the `UNLICENSE` file or [unlicense.org](http://unlicense.org).
