//! snes-recomp as a library: the static recompiler (ROM, disassembly, discovery, graph, C generator), the game
//! profiles, the port builder and the generation of a game's C in memory (`generate`), which is what the command line
//! programs use and what a build on another device (Android) calls through its C interface (`generate::ffi`).

pub mod codegen;
#[cfg(test)]
mod cctest;
pub mod config;
pub mod crc32;
pub mod disasm;
pub mod discover;
pub mod download;
pub mod games;
pub mod generate;
pub mod graph;
#[cfg(test)]
mod headless;
pub mod opcodes;
pub mod port;
pub mod rom;
#[cfg(test)]
mod rtlib;
#[cfg(test)]
mod spctest;
pub mod toolchain;
