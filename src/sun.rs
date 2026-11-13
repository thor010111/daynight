// SunCalc equations by Vladimir Agafonkin, ported from the original MIT Swift implementation.
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct SunPosition {
    pub altitude: f64,
    pub azimuth: f64,
}

pub fn position(latitude: f64, longitude: f64, unix_seconds: f64) -> SunPosition {
    let rad = std::f64::consts::PI / 180.0;
    let days = unix_seconds / 86400.0 - 0.5 + 2440588.0 - 2451545.0;
    let m = rad * (357.5291 + 0.98560028 * days);
    let center = rad * (1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin());
    let l = m + center + rad * 102.9372 + std::f64::consts::PI;
    let (sin_l, cos_l) = l.sin_cos();
    let (sin_e, cos_e) = (rad * 23.4397).sin_cos();
    let dec = (sin_e * sin_l).asin();
    let ra = (sin_l * cos_e).atan2(cos_l);
    let h = rad * (280.16 + 360.9856235 * days + longitude) - ra;
    let (sin_h, cos_h) = h.sin_cos();
    let (sin_phi, cos_phi) = (latitude * rad).sin_cos();
    let (sin_dec, cos_dec) = dec.sin_cos();
    SunPosition {
        altitude: (sin_phi * sin_dec + cos_phi * cos_dec * cos_h)
            .asin()
            .to_degrees(),
        azimuth: (sin_h
            .atan2(cos_h * sin_phi - dec.tan() * cos_phi)
            .to_degrees()
            + 180.0)
            .rem_euclid(360.0),
    }
}
