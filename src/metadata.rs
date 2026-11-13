use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureInfo {
    pub file_name: String,
    pub is_primary: Option<bool>,
    pub is_for_light: Option<bool>,
    pub is_for_dark: Option<bool>,
    pub altitude: Option<f64>,
    pub azimuth: Option<f64>,
    pub time: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Appearance {
    pub d: usize,
    pub l: usize,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SolarItem {
    pub a: f64,
    pub z: f64,
    pub i: usize,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TimeItem {
    pub t: f64,
    pub i: usize,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Sequence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ap: Option<Appearance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si: Option<Vec<SolarItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ti: Option<Vec<TimeItem>>,
}

pub struct Plan<'a> {
    pub images: Vec<&'a str>,
    pub key: &'static str,
    pub payload: Vec<u8>,
}
impl<'a> Plan<'a> {
    pub fn new(pictures: &'a [PictureInfo]) -> Result<Self> {
        if pictures.is_empty() {
            bail!("Wallpaper must contain at least one image");
        }
        let mut images = Vec::with_capacity(pictures.len());
        let mut indices = HashMap::with_capacity(pictures.len());
        // Stable primary-first partition and deduplication, with O(1) index lookups.
        for primary in [true, false] {
            for p in pictures
                .iter()
                .filter(|p| p.is_primary.unwrap_or(false) == primary)
            {
                if p.file_name.is_empty() {
                    bail!("Image fileName must not be empty");
                }
                indices.entry(p.file_name.as_str()).or_insert_with(|| {
                    let index = images.len();
                    images.push(p.file_name.as_str());
                    index
                });
            }
        }
        let mut sequence = Sequence::default();
        for p in pictures {
            let i = indices[p.file_name.as_str()];
            if p.is_for_light.is_some() || p.is_for_dark.is_some() {
                let ap = sequence.ap.get_or_insert_with(Appearance::default);
                if p.is_for_light == Some(true) {
                    ap.l = i;
                }
                if p.is_for_dark == Some(true) {
                    ap.d = i;
                }
            }
            if let (Some(a), Some(z)) = (p.altitude, p.azimuth) {
                if !a.is_finite() || !z.is_finite() {
                    bail!("Solar coordinates must be finite");
                }
                sequence
                    .si
                    .get_or_insert_with(Vec::new)
                    .push(SolarItem { a, z, i });
            }
            if let Some(time) = &p.time {
                sequence.ti.get_or_insert_with(Vec::new).push(TimeItem {
                    t: time_fraction(time)?,
                    i,
                });
            }
        }
        let mut payload = Vec::new();
        let key = if sequence.si.is_some() {
            "solar"
        } else if sequence.ti.is_some() {
            "h24"
        } else {
            "apr"
        };
        if key == "apr" {
            plist::to_writer_binary(
                &mut payload,
                &sequence
                    .ap
                    .context("Appearance wallpaper requires a light or dark selection")?,
            )?;
        } else {
            plist::to_writer_binary(&mut payload, &sequence)?;
        }
        Ok(Self {
            images,
            key,
            payload,
        })
    }
    pub fn base64(&self) -> String {
        STANDARD.encode(&self.payload)
    }
}

pub fn decode(value: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(value)
        .context("Invalid base64 wallpaper metadata")
}

/// Match Swift's local calendar hour/minute semantics, including explicit offsets.
/// Seconds intentionally do not contribute to Apple's h24 schedule.
pub fn time_fraction(value: &str) -> Result<f64> {
    let time = if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        date.with_timezone(&Local).time()
    } else if let Ok(date) = DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%z") {
        date.with_timezone(&Local).time()
    } else if let Ok(date) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        date.time()
    } else if let Ok(time) = NaiveTime::parse_from_str(value, "%H:%M:%S") {
        time
    } else {
        // DateFormatter uses a reference date for time-only values with offsets.
        let date = DateTime::parse_from_str(&format!("2000-01-01T{value}"), "%Y-%m-%dT%H:%M:%S%z")
            .or_else(|_| DateTime::parse_from_rfc3339(&format!("2000-01-01T{value}")))
            .with_context(|| format!("Invalid time: {value}"))?;
        date.with_timezone(&Local).time()
    };
    Ok(time.hour() as f64 / 24.0 + time.minute() as f64 / 1440.0)
}

pub fn parse_date(value: &str) -> Result<DateTime<Local>> {
    if let Ok(date) = DateTime::parse_from_rfc3339(value) {
        return Ok(date.with_timezone(&Local));
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y:%m:%d %H:%M:%S"] {
        if let Ok(date) = NaiveDateTime::parse_from_str(value, format) {
            return Local
                .from_local_datetime(&date)
                .single()
                .context("Ambiguous or nonexistent local capture time");
        }
    }
    bail!("Unsupported creation date: {value}")
}
