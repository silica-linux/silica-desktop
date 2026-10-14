//! Sunrise and sunset for the schedules that follow the sun (the automatic appearance, the night
//! light), from the time zone's coordinates: no location service is needed.
//!
//! - The place: the time zone (`/etc/localtime` → `Europe/Rome`) looked up in tzdata's
//!   `zone1970.tab` (or `zone.tab`), which gives each zone its principal city's coordinates.
//! - The times: NOAA's solar equations (equation of time and declination from the day of the
//!   year), in UTC minutes; good to a couple of minutes, plenty for a schedule.
//! - The local time: the UTC offset from `date +%z`, read again at every check.

use std::path::Path;

/// Minutes in a day.
pub const DAY: i64 = 24 * 60;

/// Latitude and longitude in degrees (north and east positive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
}

/// ISO 6709 coordinates as tzdata writes them: `+4154+01229` or `+404251-0740023`.
pub fn parse_iso6709(text: &str) -> Option<Place> {
    let split = text.get(1..)?.find(['+', '-'])?.checked_add(1)?;
    let (lat, lon) = (text.get(..split)?, text.get(split..)?);
    let angle = |s: &str, degree_digits: usize| -> Option<f64> {
        let sign = match s.get(..1)? {
            "+" => 1.0,
            "-" => -1.0,
            _ => return None,
        };
        let digits = s.get(1..)?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let degrees: f64 = digits.get(..degree_digits)?.parse().ok()?;
        let rest = digits.get(degree_digits..)?;
        let minutes: f64 = rest.get(..2)?.parse().ok()?;
        let seconds: f64 = match rest.len() {
            2 => 0.0,
            4 => rest.get(2..4)?.parse().ok()?,
            _ => return None,
        };
        Some(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
    };
    Some(Place {
        lat: angle(lat, 2)?,
        lon: angle(lon, 3)?,
    })
}

/// The coordinates of `zone` in a `zone1970.tab`/`zone.tab` text.
pub fn zone_place(table: &str, zone: &str) -> Option<Place> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut fields = line.split('\t');
            let _countries = fields.next()?;
            let coordinates = fields.next()?;
            (fields.next()? == zone).then(|| parse_iso6709(coordinates))?
        })
}

/// The system's time zone name: `$TZ` (`:Europe/Rome` or `Europe/Rome`), otherwise the target
/// of `/etc/localtime` after `zoneinfo/`.
pub fn local_zone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if tz.contains('/') && !tz.starts_with('/') {
            return Some(tz.to_owned());
        }
    }
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let text = target.to_string_lossy().into_owned();
    let (_, zone) = text.split_once("zoneinfo/")?;
    Some(zone.to_owned())
}

/// The place of the system's time zone, if tzdata knows it.
pub fn local_place() -> Option<Place> {
    let zone = local_zone()?;
    ["zone1970.tab", "zone.tab"].iter().find_map(|name| {
        let table = std::fs::read_to_string(Path::new("/usr/share/zoneinfo").join(name)).ok()?;
        zone_place(&table, &zone)
    })
}

/// Days since 1970-01-01 → (year, month, day) (proleptic Gregorian).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// (year, month, day) → days since 1970-01-01.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The day of the year (1–366) of a day since the epoch.
pub fn day_of_year(days: i64) -> u32 {
    let (year, _, _) = civil_from_days(days);
    u32::try_from(days - days_from_civil(year, 1, 1) + 1).unwrap_or(1)
}

/// Sunrise and sunset on the day of the year `doy`, in minutes from that day's UTC midnight
/// (they may fall outside 0–1440 far from Greenwich). `None` in polar day or night.
pub fn sun_times(place: Place, doy: u32) -> Option<(f64, f64)> {
    use std::f64::consts::PI;
    let gamma = 2.0 * PI / 365.0 * (f64::from(doy) - 1.0);
    let eqtime = 229.18
        * (0.000_075 + 0.001_868 * gamma.cos()
            - 0.032_077 * gamma.sin()
            - 0.014_615 * (2.0 * gamma).cos()
            - 0.040_849 * (2.0 * gamma).sin());
    let decl = 0.006_918 - 0.399_912 * gamma.cos() + 0.070_257 * gamma.sin()
        - 0.006_758 * (2.0 * gamma).cos()
        + 0.000_907 * (2.0 * gamma).sin()
        - 0.002_697 * (3.0 * gamma).cos()
        + 0.001_48 * (3.0 * gamma).sin();
    let lat = place.lat.to_radians();
    let cos_ha = 90.833_f64.to_radians().cos() / (lat.cos() * decl.cos()) - lat.tan() * decl.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return None;
    }
    let ha = cos_ha.acos().to_degrees();
    let sunrise = 720.0 - 4.0 * (place.lon + ha) - eqtime;
    let sunset = 720.0 - 4.0 * (place.lon - ha) - eqtime;
    Some((sunrise, sunset))
}

/// Whether it is night at `now` (Unix seconds) with the local UTC offset `offset` (seconds):
/// between sunset and sunrise at `place`, or between 19:00 and 07:00 local time without a place
/// or in polar day and night.
pub fn is_night(place: Option<Place>, now: i64, offset: i64) -> bool {
    let local = now + offset;
    let days = local.div_euclid(86_400);
    let fallback = || {
        let minute = local.rem_euclid(86_400) / 60;
        in_window(minute, 19 * 60, 7 * 60)
    };
    let Some(place) = place else {
        return fallback();
    };
    let Some((sunrise, sunset)) = sun_times(place, day_of_year(days)) else {
        return fallback();
    };
    // The local day's UTC midnight: the times are counted from there.
    let midnight = days * 86_400;
    let at = |minutes: f64| midnight + (minutes * 60.0).round() as i64;
    let (rise, set) = (at(sunrise), at(sunset));
    // `now` against the local day's sunrise and sunset.
    !(rise <= now && now < set)
}

/// Whether `minute` (of the day) is in the window from `from` to `to`, which may cross midnight.
pub fn in_window(minute: i64, from: i64, to: i64) -> bool {
    if from == to {
        return false;
    }
    if from < to {
        (from..to).contains(&minute)
    } else {
        minute >= from || minute < to
    }
}

/// `HH:MM` → minutes of the day.
pub fn parse_clock(text: &str) -> Option<i64> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

/// `+0200` / `-0530` → seconds.
pub fn parse_offset(text: &str) -> Option<i64> {
    let text = text.trim();
    let sign = match text.get(..1)? {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let digits = text.get(1..5)?;
    let h: i64 = digits.get(..2)?.parse().ok()?;
    let m: i64 = digits.get(2..)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// The local UTC offset now, from `date +%z`; 0 if it cannot be read.
pub async fn local_offset() -> i64 {
    let output = async_process::Command::new("date")
        .arg("+%z")
        .stdin(async_process::Stdio::null())
        .stderr(async_process::Stdio::null())
        .output()
        .await;
    output
        .ok()
        .and_then(|o| parse_offset(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(0)
}

/// Seconds since the epoch.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_from_tzdata() {
        let rome = parse_iso6709("+4154+01229").unwrap();
        assert!((rome.lat - 41.9).abs() < 1e-9 && (rome.lon - 12.483_333).abs() < 1e-5);
        let ny = parse_iso6709("+404251-0740023").unwrap();
        assert!((ny.lat - 40.714_166).abs() < 1e-5 && (ny.lon + 74.006_388).abs() < 1e-5);
        assert_eq!(parse_iso6709("4154+01229"), None);
        assert_eq!(parse_iso6709("+41x4+01229"), None);
        let table = "# comment\nIT,SM,VA\t+4154+01229\tEurope/Rome\nUS\t+404251-0740023\tAmerica/New_York\tEastern\n";
        assert_eq!(zone_place(table, "Europe/Rome"), Some(rome));
        assert_eq!(zone_place(table, "America/New_York"), Some(ny));
        assert_eq!(zone_place(table, "Mars/Base"), None);
    }

    #[test]
    fn calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_376), (2025, 10, 15));
        assert_eq!(day_of_year(0), 1);
        assert_eq!(days_from_civil(2025, 10, 15), 20_376);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        // 2024-12-31: a leap year.
        assert_eq!(day_of_year(20_088), 366);
        assert_eq!(day_of_year(20_089), 1);
    }

    #[test]
    fn rome_sunrise_and_sunset() {
        let rome = Place {
            lat: 41.9,
            lon: 12.4833,
        };
        // 21 June: sunrise ~03:36 UTC (05:36 CEST), sunset ~18:48 UTC (20:48 CEST).
        let (rise, set) = sun_times(rome, 172).unwrap();
        assert!((rise - 216.0).abs() < 6.0, "{rise}");
        assert!((set - 1128.0).abs() < 6.0, "{set}");
        // 21 December: ~06:34 UTC and ~15:42 UTC.
        let (rise, set) = sun_times(rome, 355).unwrap();
        assert!((rise - 394.0).abs() < 6.0, "{rise}");
        assert!((set - 942.0).abs() < 6.0, "{set}");
        // The North Pole in June: no sunset.
        assert_eq!(
            sun_times(
                Place {
                    lat: 89.0,
                    lon: 0.0
                },
                172
            ),
            None
        );
    }

    #[test]
    fn night_follows_the_sun_or_the_clock() {
        let rome = Some(Place {
            lat: 41.9,
            lon: 12.4833,
        });
        // 2025-06-21 (day 20_260): 12:00 UTC is day, 22:00 UTC and 02:00 UTC are night.
        let day = 20_260 * 86_400;
        assert!(!is_night(rome, day + 12 * 3600, 7200));
        assert!(is_night(rome, day + 22 * 3600, 7200));
        assert!(is_night(rome, day + 2 * 3600, 7200));
        // Without a place: 19:00–07:00 local.
        assert!(is_night(None, day + 18 * 3600, 3600));
        assert!(!is_night(None, day + 17 * 3600, 3600));
        assert!(in_window(23 * 60, 22 * 60, 7 * 60));
        assert!(!in_window(12 * 60, 22 * 60, 7 * 60));
        assert!(in_window(12 * 60, 9 * 60, 17 * 60));
        assert!(!in_window(5, 5, 5));
        assert_eq!(parse_clock("07:30"), Some(450));
        assert_eq!(parse_clock("24:00"), None);
        assert_eq!(parse_offset("+0200\n"), Some(7200));
        assert_eq!(parse_offset("-0530"), Some(-19_800));
        assert_eq!(parse_offset("x"), None);
    }
}
