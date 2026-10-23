mod codegen;
#[cfg(test)]
mod cctest;
mod config;
mod crc32;
mod disasm;
mod discover;
mod games;
mod graph;
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
  snes-recomp discover <rom> [addr] [--config <file>] [--unresolved] [--widths]
  snes-recomp graph <rom> [addr] [--config <file>]
  snes-recomp codegen <rom> <addr> [--depth N] [-o <file.c>] [--config <file>]

<addr> is hex, as BB:AAAA or BBAAAA.
discover loads games/<id>.toml for known ROMs (override with --config). Without an <addr> it
starts from the config's entries, or the reset vector if there are none.
--unresolved lists indirect jumps/calls that still need a table in the config.
--widths lists M/X conflicts, unpaired PLPs and calls whose callee never returns.
graph prints totals for functions and basic blocks; with <addr> it details the function containing it.
codegen emits C for the function at <addr> and, up to --depth levels (default 1), the functions it calls.
With -o it also writes snes_rt.h next to the file. The output derives from the ROM: keep it under out/.";

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
        if a == "--config" || a == "--depth" || a == "-o" {
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
            let cfg = load_config(&rom, config_path)?;
            let roots: Vec<(u32, Flags)> = match rest.first() {
                Some(s) => vec![(parse_addr(s)?, flags_from(&args))],
                None => default_roots(&rom, &cfg),
            };
            let d = discover::discover(&rom, &cfg, &roots);
            if args.iter().any(|a| a == "--widths") {
                print_widths(&d);
            } else if args.iter().any(|a| a == "--unresolved") {
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
        "graph" => {
            let cfg = load_config(&rom, config_path)?;
            let roots = default_roots(&rom, &cfg);
            let d = discover::discover(&rom, &cfg, &roots);
            let root_addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
            let g = graph::Graph::build(&d, &root_addrs);
            match rest.first() {
                Some(s) => print_function(&g, parse_addr(s)?),
                None => {
                    print_graph_summary(&d, &g);
                    Ok(())
                }
            }
        }
        "codegen" => {
            let cfg = load_config(&rom, config_path)?;
            let roots = default_roots(&rom, &cfg);
            let d = discover::discover(&rom, &cfg, &roots);
            let root_addrs: Vec<u32> = roots.iter().map(|r| r.0).collect();
            let g = graph::Graph::build(&d, &root_addrs);
            let addr = parse_addr(rest.first().ok_or(USAGE)?)?;
            let depth = option_value(&args, "--depth")
                .map(|s| s.parse::<usize>().map_err(|_| format!("bad depth: {s}")))
                .transpose()?
                .unwrap_or(1);
            let text = codegen::Emitter::new(&d, &g).generate(addr, depth)?;
            match option_value(&args, "-o") {
                Some(path) => {
                    let path = PathBuf::from(path);
                    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
                    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                    std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
                    let header = dir.join("snes_rt.h");
                    std::fs::write(&header, codegen::RUNTIME_HEADER).map_err(|e| format!("{}: {e}", header.display()))?;
                    eprintln!("wrote {} and {}", path.display(), header.display());
                }
                None => print!("{text}"),
            }
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    }
}

/// Config from `--config`, else the bundled one for a known ROM, else an empty one.
fn load_config(rom: &Rom, path: Option<String>) -> Result<Config, String> {
    match path {
        Some(p) => Config::load(p),
        None => match games::identify(rom.crc32()) {
            Some(g) => {
                let p = PathBuf::from("games").join(format!("{}.toml", g.id));
                if p.exists() { Config::load(p) } else { Ok(Config::default()) }
            }
            None => Ok(Config::default()),
        },
    }
}

/// The config's entries, or the reset vector if there are none.
fn default_roots(rom: &Rom, cfg: &Config) -> Vec<(u32, Flags)> {
    if cfg.entries.is_empty() {
        vec![(rom.vectors().emu_reset as u32, Flags::RESET)]
    } else {
        cfg.entries.iter().map(|e| (e.addr, e.flags)).collect()
    }
}

fn print_widths(d: &discover::Discovery) {
    for (a, set) in &d.flag_conflicts {
        let seen: Vec<String> = set.iter().map(|f| fmt_flags(*f)).collect();
        println!("conflict at {}: {}", fmt_addr(*a), seen.join(" / "));
    }
    for a in &d.untracked_plp {
        println!("unpaired PLP at {}", fmt_addr(*a));
    }
    for a in &d.calls_without_return {
        println!("call without return at {}", fmt_addr(*a));
    }
    println!(
        "{} conflicts, {} unpaired PLP, {} calls without return (max stack depth {})",
        d.flag_conflicts.len(),
        d.untracked_plp.len(),
        d.calls_without_return.len(),
        d.max_stack
    );
}

fn fmt_flags(f: Flags) -> String {
    format!("M={} X={}", if f.m { 8 } else { 16 }, if f.x { 8 } else { 16 })
}

fn print_graph_summary(d: &discover::Discovery, g: &graph::Graph) {
    let tail_calls: usize = g.functions.values().map(|f| f.tail_calls.len()).sum();
    let shared = g.diagnostics.iter().filter(|x| matches!(x, graph::Diagnostic::SharedBlock { .. })).count();
    let missing = g.diagnostics.len() - shared;
    println!("functions:   {}", g.functions.len());
    println!("blocks:      {}", g.blocks.len());
    println!("instrs:      {}", d.instrs.len());
    println!("tail calls:  {tail_calls}");
    println!("M/X conflicts: {} addresses reached with different register widths", d.flag_conflicts.len());
    println!("diagnostics: {} missing targets, {} shared blocks", missing, shared);
    for x in &g.diagnostics {
        match x {
            graph::Diagnostic::MissingTarget { from, to } => {
                println!("  missing target {} from block {}", fmt_addr(*to), fmt_addr(*from))
            }
            graph::Diagnostic::SharedBlock { block, owner, other } => println!(
                "  block {} owned by {} is also reachable from {}",
                fmt_addr(*block),
                fmt_addr(*owner),
                fmt_addr(*other)
            ),
        }
    }
}

fn print_function(g: &graph::Graph, addr: u32) -> Result<(), String> {
    let f = g.function_at(addr).ok_or_else(|| format!("{} is not in any function", fmt_addr(addr)))?;
    let list = |v: &std::collections::BTreeSet<u32>| v.iter().map(|a| fmt_addr(*a)).collect::<Vec<_>>().join(", ");
    println!("function {}: {} blocks, {} returns", fmt_addr(f.entry), f.blocks.len(), f.returns);
    if !f.calls.is_empty() {
        println!("  calls:      {}", list(&f.calls));
    }
    if !f.tail_calls.is_empty() {
        println!("  tail calls: {}", list(&f.tail_calls));
    }
    for start in &f.blocks {
        let b = &g.blocks[start];
        let succs: Vec<String> = b.succs.iter().map(|(a, k)| format!("{} ({:?})", fmt_addr(*a), k)).collect();
        let tail = if b.ret { "return".to_string() } else { succs.join(", ") };
        println!("  block {}..{}  -> {}", fmt_addr(b.start), fmt_addr(b.end()), tail);
    }
    Ok(())
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
