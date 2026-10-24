//! Native hardware collection. Raw identifiers are never persisted or sent.
use crate::{
    error::{Error, ErrorCode, Result},
    verification::hash,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct HardwareFingerprint(Value);
impl std::fmt::Debug for HardwareFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HardwareFingerprint([private hashes])")
    }
}
impl HardwareFingerprint {
    /// Trusted native adapter hook; never accept identifiers supplied by an end user.
    pub fn from_components(
        product: &str,
        raw: &BTreeMap<String, String>,
        is_virtual: Option<bool>,
    ) -> Result<Self> {
        let salt = hash(format!("kdpika-hardware-v1|{product}").as_bytes());
        let mut components = BTreeMap::new();
        for (name, value) in raw {
            if NAMES.contains(&name.as_str()) {
                if let Some(value) = usable(value) {
                    components.insert(
                        name.clone(),
                        hash(format!("{salt}|{name}|{value}").as_bytes()),
                    );
                }
            }
        }
        if !components.keys().any(|k| k != "cpu") {
            return Err(Error(ErrorCode::HardwareUnavailable));
        }
        Ok(Self(
            json!({"version":1,"hardware_id":combined(&components),"components":components,"is_virtual":is_virtual}),
        ))
    }
    pub(crate) fn value(&self) -> Value {
        self.0.clone()
    }
}
const NAMES: [&str; 5] = ["baseboard", "cpu", "disk", "nic", "machine"];
fn usable(s: &str) -> Option<String> {
    let v = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if v.is_empty()
        || v.chars().count() > 1024
        || [
            "unknown",
            "none",
            "default string",
            "to be filled by o.e.m.",
            "system serial number",
            "not specified",
            "not applicable",
        ]
        .contains(&v.as_str())
        || v.chars().all(|c| "0f:- ".contains(c))
    {
        None
    } else {
        Some(v)
    }
}
fn combined(c: &BTreeMap<String, String>) -> String {
    hash(
        c.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
}
fn read(p: &str) -> String {
    fs::File::open(p)
        .ok()
        .map(|f| {
            let mut s = String::new();
            let _ = f.take(65536).read_to_string(&mut s);
            s.trim().into()
        })
        .unwrap_or_default()
}
fn run(exe: &str, args: &[&str]) -> String {
    let Ok(mut child) = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return String::new();
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return String::new();
    };
    let output = thread::spawn(move || {
        let mut b = Vec::new();
        let _ = stdout.take(65537).read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.success(),
            Err(_) => break false,
            _ => {
                if start.elapsed() >= Duration::from_secs(4) {
                    break false;
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    };
    if !success {
        let _ = child.kill();
    }
    let _ = child.wait();
    let bytes = output.join().unwrap_or_default();
    if !success || bytes.len() > 65536 {
        String::new()
    } else {
        String::from_utf8(bytes).unwrap_or_default().trim().into()
    }
}
fn after(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| {
            l.trim().strip_prefix(key).map(|r| {
                r.trim()
                    .trim_start_matches('=')
                    .trim()
                    .trim_matches('"')
                    .to_string()
            })
        })
        .unwrap_or_default()
}
pub fn collect(product: &str) -> Result<HardwareFingerprint> {
    let mut raw: BTreeMap<String,String> = BTreeMap::new();
    let model;
    match std::env::consts::OS {
        "windows" => {
            let command="$ErrorActionPreference='SilentlyContinue';$b=(Get-CimInstance Win32_BaseBoard|Select-Object -First 1).SerialNumber;$c=(Get-CimInstance Win32_Processor|Select-Object -First 1).ProcessorId;$m=(Get-CimInstance Win32_ComputerSystemProduct|Select-Object -First 1).UUID;$d=(Get-Partition -DriveLetter $env:SystemDrive.Substring(0,1)|Get-Disk|Select-Object -First 1).SerialNumber;$n=(Get-NetAdapter -Physical|Where-Object Status -eq 'Up'|Sort-Object InterfaceIndex|Select-Object -First 1).MacAddress;@{baseboard=$b;cpu=$c;machine=$m;disk=$d;nic=$n;model=(Get-CimInstance Win32_ComputerSystem).Model}|ConvertTo-Json -Compress";
            let value: Value = serde_json::from_str(&run(
                "powershell.exe",
                &[
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    command,
                ],
            ))
            .unwrap_or(Value::Null);
            for n in ["baseboard", "cpu", "disk", "nic"] {
                raw.insert(n.into(), value[n].as_str().unwrap_or("").into());
            }
            if raw.get("baseboard").and_then(|v| usable(v)).is_none() {
                raw.insert(
                    "machine".into(),
                    value["machine"].as_str().unwrap_or("").into(),
                );
            }
            model = value["model"].as_str().unwrap_or("").to_string();
        }
        "macos" => {
            let registry = run("/usr/sbin/ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"]);
            raw.insert("baseboard".into(), after(&registry, "\"IOPlatformUUID\""));
            let cpu = run("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
            raw.insert(
                "cpu".into(),
                if cpu.is_empty() {
                    run("/usr/sbin/sysctl", &["-n", "hw.model"])
                } else {
                    cpu
                },
            );
            let net = run("/sbin/ifconfig", &["en0"]);
            raw.insert(
                "nic".into(),
                net.split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .find(|p| p[0] == "ether")
                    .map(|p| p[1].to_string())
                    .unwrap_or_default(),
            );
            model = registry;
        }
        "linux" => {
            let b = read("/sys/class/dmi/id/board_serial");
            let b = if b.is_empty() {
                read("/sys/class/dmi/id/product_uuid")
            } else {
                b
            };
            if usable(&b).is_none() {
                let m = read("/etc/machine-id");
                raw.insert(
                    "machine".into(),
                    if m.is_empty() {
                        read("/var/lib/dbus/machine-id")
                    } else {
                        m
                    },
                );
            }
            raw.insert("baseboard".into(), b);
            let cpu = read("/proc/cpuinfo");
            raw.insert(
                "cpu".into(),
                cpu.lines()
                    .find_map(|l| {
                        l.split_once(':')
                            .filter(|(k, _)| {
                                ["model name", "Hardware", "Serial"].contains(&k.trim())
                            })
                            .map(|(_, v)| v.trim().into())
                    })
                    .unwrap_or_else(|| std::env::consts::ARCH.into()),
            );
            for (directory, kind) in [("/sys/class/block", "disk"), ("/sys/class/net", "nic")] {
                let mut entries = fs::read_dir(directory)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                entries.sort();
                for n in entries {
                    let root = format!("{directory}/{n}");
                    let value = if kind == "disk" {
                        if ["loop", "ram", "dm-", "sr"]
                            .iter()
                            .any(|p| n.starts_with(p))
                            || fs::metadata(format!("{root}/partition")).is_ok()
                        {
                            continue;
                        }
                        read(&format!("{root}/device/serial"))
                    } else {
                        if fs::metadata(format!("{root}/device")).is_err()
                            || read(&format!("{root}/operstate")) != "up"
                        {
                            continue;
                        }
                        read(&format!("{root}/address"))
                    };
                    if usable(&value).is_some() {
                        raw.insert(kind.into(), value);
                        break;
                    }
                }
            }
            model = format!(
                "{} {}",
                read("/sys/class/dmi/id/product_name"),
                read("/sys/class/dmi/id/sys_vendor")
            );
        }
        _ => return Err(Error(ErrorCode::HardwareUnavailable)),
    }
    let text = model.to_lowercase();
    let virtual_hint = if text.trim().is_empty() {
        None
    } else {
        Some(
            [
                "vmware",
                "virtualbox",
                "virtual machine",
                "qemu",
                "kvm",
                "parallels",
                "xen",
            ]
            .iter()
            .any(|v| text.contains(v)),
        )
    };
    HardwareFingerprint::from_components(product, &raw, virtual_hint)
}
pub(crate) fn matches(binding: &Value, current: &HardwareFingerprint) -> bool {
    let Some(saved) = binding["components"].as_object() else {
        return false;
    };
    let mut components = BTreeMap::new();
    for (k, v) in saved {
        let Some(v) = v.as_str() else { return false };
        if !NAMES.contains(&k.as_str())
            || v.len() != 64
            || !v
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return false;
        }
        components.insert(k.clone(), v.into());
    }
    if components.is_empty()
        || binding["version"] != 1
        || binding["hardware_id"] != combined(&components)
    {
        return false;
    }
    let actual = &current.0["components"];
    match binding["policy"]["mode"].as_str() {
        Some("strict") => binding["hardware_id"] == current.0["hardware_id"],
        Some("fuzzy") => {
            let Some(min) = binding["policy"]["min_matches"]
                .as_u64()
                .filter(|v| (1..=5).contains(v))
            else {
                return false;
            };
            !["baseboard", "cpu"]
                .iter()
                .any(|k| saved.contains_key(*k) && saved[*k] != actual[*k])
                && saved
                    .iter()
                    .filter(|(k, v)| *v == &actual[k.as_str()])
                    .count()
                    >= components.len().min(min as usize)
        }
        _ => false,
    }
}
