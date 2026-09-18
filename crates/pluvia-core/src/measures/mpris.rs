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

    fn query_mpris_data(&mut self) -> Option<NowPlayingData> {
        if self.connection.is_none() {
            self.connection = Connection::session().ok();
        }
        let conn = self.connection.as_ref()?;

        let destination = if let Some(ref target) = self.player_name {
            if target.starts_with("org.mpris.MediaPlayer2.") {
                target.clone()
            } else {
                format!("org.mpris.MediaPlayer2.{}", target)
            }
        } else {
            let dbus_proxy = Proxy::new(
                &conn,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .ok()?;
            let names: Vec<String> = dbus_proxy.call("ListNames", &()).ok()?;
            names
                .into_iter()
                .find(|n| n.starts_with("org.mpris.MediaPlayer2."))?
        };

        let player_proxy = Proxy::new(
            &conn,
            destination,
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
        let cover = metadata
            .get("mpris:artUrl")
            .and_then(extract_string)
            .unwrap_or_default();
        let artist = metadata.get("xesam:artist").map(extract_artist).unwrap_or_default();

        let duration_us: u64 = metadata
            .get("mpris:length")
            .and_then(extract_u64)
            .unwrap_or(0);
        let duration = duration_us as f64 / 1_000_000.0;

        let position_us: i64 = player_proxy.get_property("Position").unwrap_or(0);
        let position = position_us as f64 / 1_000_000.0;

        Some(NowPlayingData {
            title,
            artist,
            album,
            cover,
            state,
            status,
            duration,
            position,
        })
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
}
