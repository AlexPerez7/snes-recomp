//! Known ROM dumps.

pub struct GameProfile {
    /// Used to find the per-game config at `games/<id>.toml`.
    pub id: &'static str,
    pub name: &'static str,
    pub crc32: u32,
}

/// The USA release; a copier header is stripped before hashing.
pub const ALTTP_USA: GameProfile = GameProfile {
    id: "alttp_usa",
    name: "The Legend of Zelda: A Link to the Past (USA)",
    crc32: 0x777A_AC2F,
};

pub const KNOWN: &[GameProfile] = &[ALTTP_USA];

pub fn identify(crc: u32) -> Option<&'static GameProfile> {
    KNOWN.iter().find(|g| g.crc32 == crc)
}
