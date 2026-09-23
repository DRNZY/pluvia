use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::collections::HashMap;
use std::ops::Deref;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

/// Media property to extract from MPRIS player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerType {
    Title,
    Artist,
    Album,
    Cover,
    State,
    Status,
    Duration,
    Position,
    Progress,
    Volume,
}

/// Extracted playback metadata.
#[derive(Debug, Clone, Default)]
pub struct NowPlayingData {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub state: u32,
    pub status: u32,
    pub duration: f64,
    pub position: f64,
    pub volume: f64,
}

/// Measure connecting to MPRIS D-Bus services (`org.mpris.MediaPlayer2.*`).
#[derive(Debug)]
pub struct NowPlayingMeasure {
    player_type: PlayerType,
    player_name: Option<String>,
    current_value: MeasureValue,
    mock_data: Option<NowPlayingData>,
    connection: Option<Connection>,
}

impl NowPlayingMeasure {
    /// Create a new `NowPlayingMeasure` for a specific `PlayerType`.
    pub fn new(player_type: PlayerType) -> Self {
        Self {
            player_type,
            player_name: None,
            current_value: match player_type {
                PlayerType::Title | PlayerType::Artist | PlayerType::Album | PlayerType::Cover => {
                    MeasureValue::String(String::new())
                }
                _ => MeasureValue::Number(0.0),
            },
            mock_data: None,
            connection: None,
        }
    }

    /// Specify a target player service name (e.g. `"spotify"` or `"vlc"`).
    pub fn with_player(mut self, player: &str) -> Self {
        self.player_name = Some(player.to_string());
        self
    }

    /// Provide mock playback data (useful for deterministic tests).
    pub fn with_mock_data(mut self, data: NowPlayingData) -> Self {
        self.mock_data = Some(data);
        self
    }

    /// Set mock data dynamically.
    pub fn set_mock_data(&mut self, data: Option<NowPlayingData>) {
        self.mock_data = data;
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let ptype_str = config
            .get("playertype")
            .unwrap_or("title")
            .to_ascii_lowercase();

        let player_type = match ptype_str.as_str() {
            "artist" => PlayerType::Artist,
            "album" => PlayerType::Album,
            "cover" => PlayerType::Cover,
            "state" => PlayerType::State,
            "status" => PlayerType::Status,
            "duration" => PlayerType::Duration,
            "position" => PlayerType::Position,
            "progress" => PlayerType::Progress,
            "volume" => PlayerType::Volume,
            _ => PlayerType::Title,
        };

        let player_name = config.get("playername").map(|s| {
            let trimmed = s.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                &trimmed[1..trimmed.len() - 1]
            } else {
                trimmed
            }
            .to_string()
        });

        Self {
            player_type,
            player_name,
            current_value: match player_type {
                PlayerType::Title | PlayerType::Artist | PlayerType::Album | PlayerType::Cover => {
                    MeasureValue::String(String::new())
                }
                _ => MeasureValue::Number(0.0),
            },
            mock_data: None,
            connection: None,
        }
    }

    fn get_player_destination(&mut self) -> Option<String> {
        if self.connection.is_none() {
            self.connection = Connection::session().ok();
        }
        let conn = self.connection.as_ref()?;

        if let Some(ref target) = self.player_name {
            if target.starts_with("org.mpris.MediaPlayer2.") {
                Some(target.clone())
            } else {
                Some(format!("org.mpris.MediaPlayer2.{}", target))
            }
        } else {
            let dbus_proxy = Proxy::new(
                conn,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .ok()?;
            let names: Vec<String> = dbus_proxy.call("ListNames", &()).ok()?;
            names
                .into_iter()
                .find(|n| n.starts_with("org.mpris.MediaPlayer2."))
        }
    }

    fn query_mpris_data(&mut self) -> Option<NowPlayingData> {
        let destination = self.get_player_destination()?;
        let conn = self.connection.as_ref()?;

        let player_proxy = Proxy::new(
            conn,
            destination.as_str(),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
        )
        .ok()?;

        let status_str: String = player_proxy
            .get_property("PlaybackStatus")
            .unwrap_or_else(|_| "Stopped".to_string());
        let state = match status_str.to_ascii_lowercase().as_str() {
            "playing" => 1,
            "paused" => 2,
            _ => 0,
        };
        let status = 1;

        let metadata: HashMap<String, OwnedValue> =
            player_proxy.get_property("Metadata").unwrap_or_default();

        let title = metadata
            .get("xesam:title")
            .and_then(extract_string)
            .unwrap_or_default();
        let album = metadata
            .get("xesam:album")
            .and_then(extract_string)
            .unwrap_or_default();
        let raw_cover = metadata
            .get("mpris:artUrl")
            .and_then(extract_string)
            .unwrap_or_default();

        let cover = normalize_cover_art_url(&raw_cover);
        let artist = metadata.get("xesam:artist").map(extract_artist).unwrap_or_default();

        let duration_us: u64 = metadata
            .get("mpris:length")
            .and_then(extract_u64)
            .unwrap_or(0);
        let duration = duration_us as f64 / 1_000_000.0;

        let position_us: i64 = player_proxy.get_property("Position").unwrap_or(0);
        let position = position_us as f64 / 1_000_000.0;

        let vol_f64: f64 = player_proxy.get_property("Volume").unwrap_or(1.0);
        let volume = (vol_f64 * 100.0).clamp(0.0, 100.0);

        Some(NowPlayingData {
            title,
            artist,
            album,
            cover,
            state,
            status,
            duration,
            position,
            volume,
        })
    }

    /// Dispatches playback control commands (Play, Pause, PlayPause, Next, Previous, Stop, etc.).
    pub fn execute_command(&mut self, cmd: &str) -> bool {
        let trimmed = cmd.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(ref mut mock) = self.mock_data {
            match lower.as_str() {
                "playpause" | "toggleplaypause" => {
                    mock.state = if mock.state == 1 { 2 } else { 1 };
                    return true;
                }
                "play" => {
                    mock.state = 1;
                    return true;
                }
                "pause" => {
                    mock.state = 2;
                    return true;
                }
                "stop" => {
                    mock.state = 0;
                    mock.position = 0.0;
                    return true;
                }
                "next" => {
                    mock.position = 0.0;
                    return true;
                }
                "previous" | "prev" => {
                    mock.position = 0.0;
                    return true;
                }
                _ => return true,
            }
        }

        let destination = match self.get_player_destination() {
            Some(d) => d,
            None => return false,
        };
        let conn = match self.connection.as_ref() {
            Some(c) => c,
            None => return false,
        };

        let player_proxy = match Proxy::new(
            conn,
            destination.as_str(),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
        ) {
            Ok(p) => p,
            Err(_) => return false,
        };

        match lower.as_str() {
            "playpause" | "toggleplaypause" => {
                let _: Result<(), _> = player_proxy.call("PlayPause", &());
                true
            }
            "play" => {
                let _: Result<(), _> = player_proxy.call("Play", &());
                true
            }
            "pause" => {
                let _: Result<(), _> = player_proxy.call("Pause", &());
                true
            }
            "stop" => {
                let _: Result<(), _> = player_proxy.call("Stop", &());
                true
            }
            "next" => {
                let _: Result<(), _> = player_proxy.call("Next", &());
                true
            }
            "previous" | "prev" => {
                let _: Result<(), _> = player_proxy.call("Previous", &());
                true
            }
            _ => {
                if lower.starts_with("setposition") {
                    let rest = trimmed["setposition".len()..].trim();
                    if let Ok(pos_secs) = rest.parse::<f64>() {
                        let pos_us = (pos_secs * 1_000_000.0) as i64;
                        let _: Result<(), _> = player_proxy.call("SetPosition", &("/org/mpris/MediaPlayer2/TrackList/NoTrack", pos_us));
                    }
                    true
                } else if lower.starts_with("setvolume") {
                    let rest = trimmed["setvolume".len()..].trim();
                    if let Ok(vol_pct) = rest.parse::<f64>() {
                        let vol_frac = (vol_pct / 100.0).clamp(0.0, 1.0);
                        let _: Result<(), _> = player_proxy.set_property("Volume", vol_frac);
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

fn normalize_cover_art_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(stripped) = trimmed.strip_prefix("file://") {
        percent_decode_str(stripped)
    } else {
        trimmed.to_string()
    }
}

fn percent_decode_str(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                if let (Some(d1), Some(d2)) = (hex_digit(h1), hex_digit(h2)) {
                    bytes.push((d1 << 4) | d2);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn extract_string(val: &OwnedValue) -> Option<String> {
    match val.deref() {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

fn extract_u64(val: &OwnedValue) -> Option<u64> {
    match val.deref() {
        Value::U64(n) => Some(*n),
        Value::I64(n) => Some(*n as u64),
        Value::U32(n) => Some(*n as u64),
        _ => None,
    }
}

fn extract_artist(val: &OwnedValue) -> String {
    match val.deref() {
        Value::Str(s) => s.to_string(),
        Value::Array(arr) => {
            let artists: Vec<String> = arr
                .iter()
                .filter_map(|v| match v {
                    Value::Str(s) => Some(s.to_string()),
                    _ => None,
                })
                .collect();
            artists.join(", ")
        }
        _ => String::new(),
    }
}

impl Measure for NowPlayingMeasure {
    fn update(&mut self) -> MeasureValue {
        let data = if let Some(ref mock) = self.mock_data {
            Some(mock.clone())
        } else {
            self.query_mpris_data()
        };

        let val = match data {
            Some(d) => match self.player_type {
                PlayerType::Title => MeasureValue::String(d.title),
                PlayerType::Artist => MeasureValue::String(d.artist),
                PlayerType::Album => MeasureValue::String(d.album),
                PlayerType::Cover => MeasureValue::String(d.cover),
                PlayerType::State => MeasureValue::Number(d.state as f64),
                PlayerType::Status => MeasureValue::Number(d.status as f64),
                PlayerType::Duration => MeasureValue::Number(d.duration),
                PlayerType::Position => MeasureValue::Number(d.position),
                PlayerType::Volume => MeasureValue::Number(d.volume),
                PlayerType::Progress => {
                    let pct = if d.duration > 0.0 {
                        (d.position / d.duration) * 100.0
                    } else {
                        0.0
                    };
                    MeasureValue::Number(pct.clamp(0.0, 100.0))
                }
            },
            None => match self.player_type {
                PlayerType::Title | PlayerType::Artist | PlayerType::Album | PlayerType::Cover => {
                    MeasureValue::String(String::new())
                }
                _ => MeasureValue::Number(0.0),
            },
        };

        self.current_value = val;
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn command(&mut self, cmd: &str) {
        self.execute_command(cmd);
    }
}
