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
    should_lshift := false
    should_rshift := false
    loop {
        evts := next_events(evdevs)
        for i len(evts) {
            evt := evts[i]
            xcape(mut should_esc, evt, KEY_CAPSLOCK(), [KEY_ESC()])
            xcape(mut should_lshift, evt, KEY_LEFTSHIFT(), [KEY_LEFTSHIFT(), KEY_9()])
            xcape(mut should_rshift, evt, KEY_RIGHTSHIFT(), [KEY_LEFTSHIFT(), KEY_0()])
        }
    }
}
```

Check out the source of the standard library in [src/stdlib.dyon](https://github.com/myfreeweb/evscript/blob/master/src/stdlib.dyon) to see how `xcape` is implemented!

And you can run it like this:

```bash
evscript -f my_script.dyon -d /dev/input/event2 /dev/input/event3
```

For now, only an explicit list of devices is supported.
There is no hotplugging mode yet.
You can setup devd/udev to run evscript on each plugged device, but that would run independent instances, not one instance that sees events from all the devices.

Also, you can run it in expression mode, kinda like you would run `xdotool` to just press a key:

```bash
evscript -e "for i 4 { click_key_chord([KEY_LEFTCTRL(), KEY_C()]); sleep(0.3); }"
```

Dyon does not have semicolons, so this mode replaces `);` and `};` with `)\n` and `}\n`.

## Contributing

By participating in this project you agree to follow the [Contributor Code of Conduct](https://www.contributor-covenant.org/version/1/4/).

[The list of contributors is available on GitHub](https://github.com/myfreeweb/evscript/graphs/contributors).

## License

This is free and unencumbered software released into the public domain.  
For more information, please refer to the `UNLICENSE` file or [unlicense.org](http://unlicense.org).
