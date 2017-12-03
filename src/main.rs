extern crate clap;
#[macro_use]
extern crate dyon;
#[macro_use]
extern crate evdev;
extern crate nix;
extern crate range;
extern crate rusty_sandbox;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::{env, fs, io, path};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use evdev::{data, raw, uinput, Device};
use nix::unistd;
use nix::poll::{poll, EventFlags, PollFd, POLLIN};
use dyon::{error, load_str, Array, Dfn, FnIndex, Lt, Module, Runtime, RustObject, Type, Variable};
use dyon::ast::Current;
use range::Range;

macro_rules! module_add {
    ($mod:ident << $fun:ident [$($lt:expr),*] [$($ty:expr),*] $ret:expr) => {
        $mod.add(
            Arc::new(stringify!($fun).into()),
            $fun,
            Dfn { lts: vec![$($lt),*], tys: vec![$($ty),*], ret: $ret }
        )
    }
}

macro_rules! wrap_var {
    (rustobj, $value:expr) => {
        Variable::RustObject(Arc::new(Mutex::new($value)) as RustObject)
    };
    (arr $typ:ident, $value:expr) => {
        Variable::Array(Arc::new($value.into_iter().map(|o| wrap_var!($typ, o)).collect::<Vec<_>>()) as Array)
    };
}

macro_rules! main_current_add {
    ($rt:ident $main:ident << $mutability:ident $name:ident = $var:expr) => {{
        let input_var : Arc<String> = Arc::new(stringify!($name).into());
        $main.currents.push(Current {
            name: input_var.clone(),
            source_range: Range::empty(0),
            mutable: $mutability,
        });
        $rt.local_stack.push((input_var.clone(), $rt.stack.len()));
        $rt.current_stack.push((input_var, $rt.stack.len()));
        $rt.stack.push($var);
    }};
    ($rt:ident $main:ident << $name:ident ($($typ:ident) +) = $obj:expr) => {
        main_current_add!($rt $main << false $name = wrap_var!($($typ) +, $obj))
    };
}

macro_rules! with_unwrapped_device {
    ($thing:expr, $fn:expr) => {
        match $thing {
            &Variable::RustObject(ref o) => {
                let mut guard = o.lock().expect(".lock()");
                let dev = guard.downcast_mut::<Device>().expect("downcast_mut()");
                ($fn)(dev)
            },
            ref x => panic!("What is this?? {:?}", x),
        }
    }
}

enum ScriptSource {
    Expr(String),
    Read(Box<io::Read>),
}

impl ScriptSource {
    fn read(self) -> (String, bool) {
        match self {
            ScriptSource::Expr(s) => (
                format!("fn main() ~ evdevs, uinput {{\n{}\n}}", s.replace(");", ")\n").replace("};", "}\n")),
                true,
            ),
            ScriptSource::Read(mut r) => {
                let mut result = String::new();
                let _ = r.read_to_string(&mut result).expect("read_to_string");
                (result, false)
            },
        }
    }
}

#[derive(Default, Deserialize)]
struct DeviceConfig {
    name: Option<String>,
    bustype: Option<u16>,
    vendor: Option<u16>,
    product: Option<u16>,
    version: Option<u16>,
}

impl Into<raw::uinput_setup> for DeviceConfig {
    fn into(self) -> raw::uinput_setup {
        let mut conf = raw::uinput_setup::default();
        conf.set_name(self.name.unwrap_or("Devicey McDeviceFace".into()))
            .expect("set_name");
        conf.id.bustype = self.bustype.unwrap_or(0x6);
        conf.id.vendor = self.vendor.unwrap_or(0x69);
        conf.id.product = self.product.unwrap_or(69);
        conf.id.version = self.version.unwrap_or(0);
        conf
    }
}

#[derive(Default, Deserialize)]
struct EventsConfig {
    keys: Option<Vec<String>>,
    // TODO: other events
}

#[derive(Default, Deserialize)]
struct ScriptConfig {
    device: Option<DeviceConfig>,
    events: EventsConfig,
}

pub struct InputEvent {
    pub kind: u32,
    pub code: u32,
    pub value: u32,
    pub device_idx: u32,
}

dyon_obj!{InputEvent { kind, code, value, device_idx }}

dyon_fn!{fn device_name(obj: RustObject) -> String {
    let mut guard = obj.lock().expect(".lock()");
    let dev = guard.downcast_mut::<Device>().expect(".downcast_mut()");
    let name = std::str::from_utf8(dev.name().to_bytes()).expect("from_utf8()").to_owned();
    name
}}

dyon_fn!{fn next_events(arr: Vec<Variable>) -> Vec<InputEvent> {
    loop {
        let mut pfds = arr.iter().map(|var| PollFd::new(
                with_unwrapped_device!(var, |dev : &mut Device| dev.fd()), POLLIN)).collect::<Vec<_>>();
        let _ = poll(&mut pfds, -1).expect("poll()");
        if let Some(i) = pfds.iter().position(|pfd| pfd.revents().unwrap_or(EventFlags::empty()).contains(POLLIN)) {
            let evts = with_unwrapped_device!(
                &arr[i as usize], |dev : &mut Device| dev.events().expect(".events()").collect::<Vec<_>>());
            return evts.iter().map(|evt| InputEvent {
                kind: evt._type as u32,
                code: evt.code as u32,
                value: evt.value as u32,
                device_idx: i as u32,
            }).collect::<Vec<_>>()
        }
    }
}}

dyon_fn!{fn emit_event(obj: RustObject, evt_v: Variable) -> bool {
    let mut guard = obj.lock().expect(".lock()");
    let dev = guard.downcast_mut::<uinput::Device>().expect(".downcast_mut()");
    if let Variable::Object(evt) = evt_v {
    match (evt.get(&Arc::new("kind".into())), evt.get(&Arc::new("code".into())), evt.get(&Arc::new("value".into()))) {
            (Some(&Variable::F64(kind, _)), Some(&Variable::F64(code, _)), Some(&Variable::F64(value, _))) => {
                let mut event = raw::input_event::default();
                event._type = kind as u16;
                event.code = code as u16;
                event.value = value as i32;
                dev.write_raw(event).expect("uinput write_raw()");
                true
            },
            x => {
                println!("WARNING: emit_event: event {:?} does not contain all of (kind, code, value) or one of them isn't a number {:?}", evt, x);
                false
            },
        }
    } else {
        println!("WARNING: emit_event: event is not an object");
        false
    }
}}

fn run_script(devs: Vec<Device>, uinput: uinput::Device, script_name: &str, script: String) {
    let mut module = Module::new();
    module_add!(module << device_name [Lt::Default] [Type::Any] Type::Text);
    module_add!(module << next_events [Lt::Default] [Type::Array(Box::new(Type::Any))] Type::Object);
    module_add!(module << emit_event [Lt::Default, Lt::Default] [Type::Any, Type::Object] Type::Bool);
    error(load_str("stdlib.dyon", Arc::new(include_str!("stdlib.dyon").into()), &mut module));
    error(load_str(script_name, Arc::new(script), &mut module));
    let mut rt = Runtime::new();
    match module.find_function(&Arc::new("main".into()), 0) {
        FnIndex::Loaded(i) => {
            let main_fun = &mut module.functions[i as usize];
            main_current_add!(rt main_fun << evdevs (arr rustobj) = devs);
            main_current_add!(rt main_fun << uinput (rustobj) = uinput);
        },
        x => panic!("Weird main function: {:?}", x),
    }
    error(rt.run(&Arc::new(module)));
}

fn drop_privileges() {
    if unistd::geteuid().is_root() {
        unistd::setgid(unistd::getgid()).expect("setegid()");
        unistd::setgroups(&[]).expect("setgroups()");
        unistd::chdir("/dev/input".into()).expect("chdir()");
        unistd::chroot("/dev/input".into()).expect("chroot()");
        unistd::setuid(unistd::getuid()).expect("setegid()");
    }
    rusty_sandbox::Sandbox::new().sandbox_this_process();
}

fn main() {
    let matches = clap::App::new("evscript")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Greg V <greg@unrelenting.technology>")
        .about("A tiny sandboxed Dyon scripting environment for evdev input devices.")
        .arg(
            clap::Arg::with_name("FILE")
                .short("f")
                .long("file")
                .takes_value(true)
                .help("The script file to run, by default - (stdin)"),
        )
        .arg(
            clap::Arg::with_name("EXPR")
                .short("e")
                .long("expr")
                .takes_value(true)
                .help("The script expression to run (inside main), overrides the file if present"),
        )
        .arg(
            clap::Arg::with_name("DEV")
                .short("d")
                .long("device")
                .takes_value(true)
                .multiple(true)
                .help("A device to get events from"),
        )
        .get_matches();

    let script_src: ScriptSource = match matches.value_of("EXPR") {
        Some(expr) => ScriptSource::Expr(expr.to_owned()),
        None => match matches.value_of("FILE") {
            Some("-") | None => ScriptSource::Read(Box::new(io::stdin())),
            Some(x) => ScriptSource::Read(Box::new(fs::File::open(x).expect("open()"))),
        },
    };

    let devs = matches
        .values_of_os("DEV")
        .map(|vs| {
            vs.map(|a| evdev::Device::open(&a).expect("evdev open()"))
                .collect::<Vec<_>>()
        })
        .unwrap_or(Vec::new());

    let uinput_path = env::var_os("EVSCRIPT_UINPUT_PATH")
        .and_then(|s| s.into_string().ok())
        .unwrap_or("/dev/uinput".to_owned());
    let ubuilder = uinput::Builder::new(&path::Path::new(&uinput_path)).expect("uinput Builder");

    drop_privileges();

    let (script, is_expr_mode) = script_src.read();

    let script_conf_str = &script
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .map(|l| format!("{}\n", l.trim_left_matches("//!")))
        .collect::<String>();
    let script_conf;
    if is_expr_mode {
        script_conf = ScriptConfig::default();
        // Just allow all keys
        uinput_ioctl!(ui_set_evbit(ubuilder.fd(), data::KEY.number())).expect("ioctl");
        for i in 0..255 {
            uinput_ioctl!(ui_set_keybit(ubuilder.fd(), i)).expect("ioctl");
        }
    } else {
        script_conf = toml::from_str(script_conf_str).expect("TOML parsing");
        if let Some(keys) = script_conf.events.keys {
            uinput_ioctl!(ui_set_evbit(ubuilder.fd(), data::KEY.number())).expect("ioctl");
            for key in keys {
                uinput_ioctl!(ui_set_keybit(
                    ubuilder.fd(),
                    data::Key::from_str(&format!("KEY_{}", key)).expect("Unknown key in script config") as i32
                )).expect("ioctl");
            }
        }
    }

    let uinput = ubuilder
        .setup(script_conf.device.unwrap_or(DeviceConfig::default()).into())
        .expect("uinput setup()");

    run_script(devs, uinput, matches.value_of("FILE").unwrap_or("-"), script);
}
