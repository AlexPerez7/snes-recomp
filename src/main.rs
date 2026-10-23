mod config;
mod crc32;
mod disasm;
mod discover;
mod games;
mod opcodes;
mod rom;

use config::{parse_addr, Config};
use disasm::{fmt_addr, Flags};
use rom::Rom;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  snes-recomp info <rom>
  snes-recomp disasm <rom> <addr> [count] [--m16] [--x16]
  snes-recomp discover <rom> [addr] [--config <file>] [--unresolved]

<addr> is hex, as BB:AAAA or BBAAAA.
discover loads games/<id>.toml for known ROMs (override with --config). Without an <addr> it
starts from the config's entries, or the reset vector if there are none.
--unresolved lists indirect jumps/calls that still need a table in the config.";

fn flags_from(args: &[String]) -> Flags {
    Flags { m: !args.iter().any(|a| a == "--m16"), x: !args.iter().any(|a| a == "--x16") }
}

/// Value following `--name`, if present.
fn option_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn run(args: Vec<String>) -> Result<(), String> {
    let cmd = args.get(1).map(String::as_str).ok_or(USAGE)?;
    let rom_path = args.get(2).ok_or(USAGE)?;
    let rom = Rom::load(rom_path)?;
    let config_path = option_value(&args, "--config");
    let mut rest: Vec<String> = Vec::new();
    let mut it = args[3..].iter();
    while let Some(a) = it.next() {
        if a == "--config" {
            it.next();
        } else if !a.starts_with("--") {
            rest.push(a.clone());
        }
    }

    match cmd {
        "info" => {
            let crc = rom.crc32();
            let h = rom.header();
            let v = rom.vectors();
            println!("size:        {} bytes{}", rom.len(), if rom.had_copier_header { " (copier header stripped)" } else { "" });
            println!("crc32:       {crc:08X}");
            match games::identify(crc) {
                Some(g) => println!("identified:  {}", g.name),
                None => println!("identified:  unknown dump"),
            }
            println!("title:       {}", h.title);
            println!("map mode:    {:02X}  type: {:02X}  country: {:02X}  version: {}", h.map_mode, h.rom_type, h.country, h.version);
            println!("reset:       00:{:04X}", v.emu_reset);
            println!("nmi:         00:{:04X} (native)  00:{:04X} (emu)", v.native_nmi, v.emu_nmi);
            println!("irq:         00:{:04X} (native)  00:{:04X} (emu)", v.native_irq, v.emu_irq);
            Ok(())
        }
        "disasm" => {
            let addr = parse_addr(rest.first().ok_or(USAGE)?)?;
            let count = rest.get(1).map(|s| s.parse::<usize>().map_err(|_| format!("bad count: {s}"))).transpose()?.unwrap_or(32);
            for ins in disasm::linear(&rom, addr, flags_from(&args), count) {
                print_instr(&rom, &ins);
            }
            Ok(())
        }
        "discover" => {
            let cfg = match config_path {
                Some(p) => Config::load(p)?,
                None => match games::identify(rom.crc32()) {
                    Some(g) => {
                        let p = PathBuf::from("games").join(format!("{}.toml", g.id));
                        if p.exists() { Config::load(p)? } else { Config::default() }
                    }
                    None => Config::default(),
                },
            };
            let roots: Vec<(u32, Flags)> = match rest.first() {
                Some(s) => vec![(parse_addr(s)?, flags_from(&args))],
                None if !cfg.entries.is_empty() => cfg.entries.iter().map(|e| (e.addr, e.flags)).collect(),
                None => vec![(rom.vectors().emu_reset as u32, Flags::RESET)],
            };
            let d = discover::discover(&rom, &cfg, &roots);
            if args.iter().any(|a| a == "--unresolved") {
                for a in &d.unresolved {
                    println!("unresolved indirect at {}", fmt_addr(*a));
                }
            } else {
                for ins in d.instrs.values() {
                    print_instr(&rom, ins);
                }
            }
            eprintln!(
                "{} instructions reachable, {} unresolved indirect sites ({} roots, {} tables configured)",
                d.instrs.len(),
                d.unresolved.len(),
                roots.len(),
                cfg.tables.len()
            );
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    }
}

fn print_instr(rom: &Rom, ins: &disasm::Instr) {
    let bytes: Vec<String> = (0..ins.len as u32)
        .map(|i| rom.read8(ins.addr + i).map_or("??".into(), |b| format!("{b:02X}")))
        .collect();
    println!("{}  {:<12}{}", fmt_addr(ins.addr), bytes.join(" "), ins.text());
}

fn main() -> ExitCode {
    match run(std::env::args().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
