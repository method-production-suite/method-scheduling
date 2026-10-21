//! Sunrise and sunset for a date and a place, in integers only (decisions.md, BYD-08).
//!
//! The method is the standard sunrise equation from positional astronomy: the days since
//! the J2000 epoch give the sun's mean anomaly, the equation of center and its ecliptic
//! longitude, from which come the time of solar noon (the equation of time folded in),
//! the sun's declination and the hour angle at which its upper edge, bent by the air,
//! touches a flat horizon (0.833 degrees below it). Good to a minute or two away from the
//! poles, which is all a call sheet needs.
//!
//! The engine has no floats (entry 3), so this is fixed-point: angles in billionths of a
//! degree, times in billionths of a day, sines and cosines scaled by 10^12, with 128-bit
//! products. Sine is a Taylor series after folding the angle into the first quadrant;
//! arccosine is a bisection on the cosine; the square root is Newton's.

use crate::Date;

/// One degree in our angle unit.
const DEG: i64 = 1_000_000_000;
/// One day in our time unit.
const DAY: i64 = 1_000_000_000;
/// The scale of sines and cosines.
const ONE: i64 = 1_000_000_000_000;
/// Pi scaled by 10^18.
const PI_E18: i128 = 3_141_592_653_589_793_238;
const E18: i128 = 1_000_000_000_000_000_000;

/// Where the sun was on one date at one place, in minutes after local midnight (the
/// place's UTC offset applied). A sunset after midnight is 1440 or more; a sunrise
/// before the previous midnight is negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sun {
    Times {
        rise: i32,
        set: i32,
    },
    /// The sun stays up all day (midnight sun).
    AlwaysUp,
    /// The sun never rises (polar night).
    AlwaysDown,
}

/// A daylight saving rule: when a location's clocks are an hour ahead of its standard
/// UTC offset (decision 193). Worked out from the date alone, with no time zone database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaylightSaving {
    /// The United States and Canada: from the second Sunday of March to the first Sunday
    /// of November, switching at 2:00 local time.
    UsCanada,
    /// The European Union: from the last Sunday of March to the last Sunday of October,
    /// switching at 1:00 UTC.
    Eu,
    /// Australia's south-east (New South Wales, Victoria, the ACT, Tasmania and South
    /// Australia): from the first Sunday of October to the first Sunday of April, switching
    /// at 2:00 local standard time. Queensland, the Northern Territory and Western Australia
    /// keep one offset all year: no rule (decision 220).
    Australia,
    /// New Zealand: from the last Sunday of September to the first Sunday of April,
    /// switching at 2:00 local standard time.
    NewZealand,
    /// Israel: from the Friday before the last Sunday of March to the last Sunday of
    /// October, switching at 2:00 local time.
    Israel,
    /// Egypt (since 2023): from the last Friday of April to the last Thursday of October,
    /// switching at midnight local time, so the clocks go back as that Thursday ends.
    Egypt,
    /// Chile, mainland (since 2023): from the first Saturday of September at midnight to
    /// the first Saturday of April at midnight, so the Sundays after them are the first
    /// and the last days changed. The Magallanes region keeps one offset all year: no rule.
    Chile,
}

impl DaylightSaving {
    pub const ALL: [DaylightSaving; 7] = [
        DaylightSaving::UsCanada,
        DaylightSaving::Eu,
        DaylightSaving::Australia,
        DaylightSaving::NewZealand,
        DaylightSaving::Israel,
        DaylightSaving::Egypt,
        DaylightSaving::Chile,
    ];

    /// The name the file and the tools use.
    pub fn name(self) -> &'static str {
        match self {
            DaylightSaving::UsCanada => "us_canada",
            DaylightSaving::Eu => "eu",
            DaylightSaving::Australia => "australia",
            DaylightSaving::NewZealand => "new_zealand",
            DaylightSaving::Israel => "israel",
            DaylightSaving::Egypt => "egypt",
            DaylightSaving::Chile => "chile",
        }
    }

    /// The rule as people read it.
    pub fn label(self) -> &'static str {
        match self {
            DaylightSaving::UsCanada => "US and Canada",
            DaylightSaving::Eu => "EU",
            DaylightSaving::Australia => "Australia (south-east)",
            DaylightSaving::NewZealand => "New Zealand",
            DaylightSaving::Israel => "Israel",
            DaylightSaving::Egypt => "Egypt",
            DaylightSaving::Chile => "Chile (mainland)",
        }
    }

    /// A rule by its file name, ignoring case.
    pub fn from_name(name: &str) -> Option<DaylightSaving> {
        Self::ALL
            .into_iter()
            .find(|r| r.name().eq_ignore_ascii_case(name.trim()))
    }

    /// The dates the clocks go forward and back in `year`.
    pub fn switch_dates(self, year: i32) -> (Date, Date) {
        match self {
            DaylightSaving::UsCanada => (nth_sunday(year, 3, 2), nth_sunday(year, 11, 1)),
            DaylightSaving::Eu => (last_sunday(year, 3), last_sunday(year, 10)),
            // Southern hemisphere: the year starts in daylight time, so the rule is read
            // across the new year: on from October (September in New Zealand) to April.
            DaylightSaving::Australia => (nth_sunday(year, 10, 1), nth_sunday(year, 4, 1)),
            DaylightSaving::NewZealand => (last_sunday(year, 9), nth_sunday(year, 4, 1)),
            DaylightSaving::Israel => (last_sunday(year, 3).add_days(-2), last_sunday(year, 10)),
            // Back as the last Thursday of October ends: the Friday is standard time.
            DaylightSaving::Egypt => (
                last_weekday(year, 4, FRIDAY),
                last_weekday(year, 10, THURSDAY).add_days(1),
            ),
            DaylightSaving::Chile => (
                first_saturday(year, 9).add_days(1),
                first_saturday(year, 4).add_days(1),
            ),
        }
    }

    /// Whether the clocks are ahead on `date`. Each switch happens in the small hours
    /// (2:00 local, or 1:00 UTC, which is before 4:00 anywhere in the EU), before any
    /// sunrise these rules' places see in March, October or November, so a whole date is
    /// one or the other: ahead from the date they go forward, not on the date they go back.
    pub fn in_effect(self, date: Date) -> bool {
        let (on, off) = self.switch_dates(date.ymd().0);
        if off < on {
            // Southern rules cross the new year: daylight time runs from the spring
            // switch of this year to the autumn switch of the next, so a date is ahead
            // either after October's date or before April's.
            return date >= on || date < off;
        }
        on <= date && date < off
    }
}

/// The `n`th Sunday (from 1) of a month.
fn nth_sunday(year: i32, month: u32, n: i32) -> Date {
    let first = Date::from_ymd(year, month, 1).expect("a real month");
    // Days from Monday: 1970-01-01 was a Thursday.
    let from_monday = (first.0 + 3).rem_euclid(7);
    first.add_days((6 - from_monday).rem_euclid(7) + 7 * (n - 1))
}

/// Days from Monday, for [`last_weekday`].
const THURSDAY: i32 = 3;
const FRIDAY: i32 = 4;

/// The first Saturday of a month.
fn first_saturday(year: i32, month: u32) -> Date {
    let first = Date::from_ymd(year, month, 1).expect("a real month");
    let from_monday = (first.0 + 3).rem_euclid(7);
    first.add_days((5 - from_monday).rem_euclid(7))
}

/// The last given weekday (days from Monday) of a month.
fn last_weekday(year: i32, month: u32, weekday: i32) -> Date {
    let sunday = last_sunday(year, month);
    // Back from the last Sunday to the weekday, unless that lands in the next month's
    // reach: the last Sunday is at most six days before the month ends.
    let back = sunday.add_days(weekday - 6);
    let ahead = back.add_days(7);
    if ahead.ymd().1 == month {
        ahead
    } else {
        back
    }
}

/// The last Sunday of a month.
fn last_sunday(year: i32, month: u32) -> Date {
    let next = if month == 12 {
        Date::from_ymd(year + 1, 1, 1)
    } else {
        Date::from_ymd(year, month + 1, 1)
    };
    let last = next.expect("a real month").add_days(-1);
    let from_monday = (last.0 + 3).rem_euclid(7);
    last.add_days(-((from_monday + 1) % 7))
}

/// Sine of an angle in billionths of a degree, scaled by 10^12.
pub fn sin(angle: i64) -> i64 {
    let a = angle.rem_euclid(360 * DEG);
    let (a, negative) = if a >= 180 * DEG {
        (a - 180 * DEG, true)
    } else {
        (a, false)
    };
    let a = if a > 90 * DEG { 180 * DEG - a } else { a };
    // Radians, scaled by 10^18.
    let x = i128::from(a) * PI_E18 / (180 * i128::from(DEG));
    let x2 = x * x / E18;
    let mut term = x;
    let mut sum = x;
    let mut k: i128 = 1;
    while term != 0 {
        term = -term * x2 / E18 / ((2 * k) * (2 * k + 1));
        sum += term;
        k += 1;
    }
    let v = (sum + 500_000) / 1_000_000;
    let v = v as i64;
    if negative {
        -v
    } else {
        v
    }
}

/// Cosine, as [`sin`].
pub fn cos(angle: i64) -> i64 {
    sin(angle + 90 * DEG)
}

/// The angle in 0 to 180 degrees (billionths) whose cosine is `c` (scaled by 10^12,
/// within -1 and 1).
pub fn acos(c: i64) -> i64 {
    let c = c.clamp(-ONE, ONE);
    let (mut lo, mut hi) = (0i64, 180 * DEG);
    // The cosine falls from 1 to -1 over the range.
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if cos(mid) > c {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Sunrise and sunset at `latitude` and `longitude` (millionths of a degree, north and
/// east positive) on `date`, as local clock minutes for a place `utc_offset` minutes
/// ahead of UTC.
pub fn sun(date: Date, latitude: i32, longitude: i32, utc_offset: i32) -> Sun {
    let j2000 = Date::from_ymd(2000, 1, 1).expect("a real date");
    // Whole days from the J2000 epoch's noon to this date's noon at Greenwich.
    let n = i64::from(date.0 - j2000.0);
    let lat = i64::from(latitude) * 1000;
    let lon = i64::from(longitude) * 1000;
    // Mean solar noon here, in billionths of a day since the epoch.
    let js = n * DAY - lon / 360;
    // Mean anomaly: 357.5291 + 0.98560028 degrees a day.
    let m = (357_529_100_000i128 + 98_560_028i128 * i128::from(js) / 100_000_000)
        .rem_euclid(360 * i128::from(DEG)) as i64;
    let (s1, s2, s3) = (
        i128::from(sin(m)),
        i128::from(sin(2 * m)),
        i128::from(sin(3 * m)),
    );
    // Equation of center: 1.9148 sin M + 0.0200 sin 2M + 0.0003 sin 3M degrees.
    let c = ((19_148 * s1 + 200 * s2 + 3 * s3) / 10_000_000) as i64;
    // Ecliptic longitude, with the perihelion's argument 102.9372 degrees.
    let l = (m + c + 180 * DEG + 102_937_200_000).rem_euclid(360 * DEG);
    // Solar transit: + 0.0053 sin M - 0.0069 sin 2L days.
    let transit = js + ((53 * s1 - 69 * i128::from(sin(2 * l))) / 10_000_000) as i64;
    // Declination, from the obliquity 23.4397 degrees.
    let sin_d = i128::from(sin(l)) * i128::from(sin(23_439_700_000)) / i128::from(ONE);
    let one = i128::from(ONE);
    let cos_d = isqrt((one * one - sin_d * sin_d).max(0) as u128) as i128;
    let sin_h = i128::from(sin(-833_000_000));
    let (sin_p, cos_p) = (i128::from(sin(lat)), i128::from(cos(lat)));
    let den = cos_p * cos_d;
    if den == 0 {
        // At a pole: up when the sun is north of the equator for the north pole.
        let up = (sin_d > 0) == (lat > 0);
        return if up { Sun::AlwaysUp } else { Sun::AlwaysDown };
    }
    let num = sin_h * one - sin_p * sin_d;
    let cos_w = num * one / den;
    if cos_w > one {
        return Sun::AlwaysDown;
    }
    if cos_w < -one {
        return Sun::AlwaysUp;
    }
    let w = acos(cos_w as i64);
    // The hour angle as a part of a day.
    let half = w / 360;
    let clock = |j: i64| -> i32 {
        // Billionths of a day from this date's UTC noon, to local minutes.
        let from_noon = i128::from(j - n * DAY);
        let minutes = (from_noon * 1440 + i128::from(DAY) / 2).div_euclid(i128::from(DAY));
        (minutes + 720 + i128::from(utc_offset)) as i32
    };
    Sun::Times {
        rise: clock(transit - half),
        set: clock(transit + half),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn sines_and_cosines() {
        assert_eq!(sin(0), 0);
        assert_eq!(sin(90 * DEG), ONE);
        assert_eq!(sin(270 * DEG), -ONE);
        assert_eq!(sin(-90 * DEG), -ONE);
        assert!((sin(30 * DEG) - ONE / 2).abs() <= 1);
        assert!((cos(60 * DEG) - ONE / 2).abs() <= 1);
        assert!(cos(180 * DEG) == -ONE);
        // sin 45 = 0.70710678118654...
        assert!((sin(45 * DEG) - 707_106_781_187).abs() <= 1);
        assert!((acos(ONE / 2) - 60 * DEG).abs() < 1_000);
        assert_eq!(acos(ONE), 0);
        assert!((acos(-ONE) - 180 * DEG).abs() < 100_000);
        assert_eq!(isqrt(1_000_000), 1000);
        assert_eq!(isqrt(999_999), 999);
    }

    /// Published times (national almanac tables as listed by public sunrise sites),
    /// rounded to the minute; we allow two minutes either way.
    #[test]
    fn known_published_times() {
        let near = |got: Sun, rise: i32, set: i32| match got {
            Sun::Times { rise: r, set: s } => {
                assert!((r - rise).abs() <= 2, "rise {r} vs {rise}");
                assert!((s - set).abs() <= 2, "set {s} vs {set}");
            }
            other => panic!("{other:?}"),
        };
        // London, midsummer 2024, BST: 04:43 and 21:21.
        near(
            sun(d("2024-06-21"), 51_507_400, -127_800, 60),
            4 * 60 + 43,
            21 * 60 + 21,
        );
        // New York, midwinter 2024, EST: 07:16 and 16:32.
        near(
            sun(d("2024-12-21"), 40_712_800, -74_006_000, -300),
            7 * 60 + 16,
            16 * 60 + 32,
        );
        // Sydney, midsummer 2024, AEDT: 05:41 and 20:05.
        near(
            sun(d("2024-12-21"), -33_868_800, 151_209_300, 660),
            5 * 60 + 41,
            20 * 60 + 5,
        );
        // Los Angeles, 2024-06-20, PDT: 05:42 and 20:08.
        near(
            sun(d("2024-06-20"), 34_052_200, -118_243_700, -420),
            5 * 60 + 42,
            20 * 60 + 8,
        );
        // Reykjavik, midsummer 2024, UTC: 02:55 and 00:03 the next day.
        near(
            sun(d("2024-06-21"), 64_146_600, -21_942_600, 0),
            2 * 60 + 55,
            24 * 60 + 3,
        );
    }

    /// Decision 193: the switch dates, from published calendars.
    #[test]
    fn daylight_saving_switch_dates() {
        let us = DaylightSaving::UsCanada;
        let eu = DaylightSaving::Eu;
        for (year, us_on, us_off, eu_on, eu_off) in [
            (2024, "2024-03-10", "2024-11-03", "2024-03-31", "2024-10-27"),
            (2025, "2025-03-09", "2025-11-02", "2025-03-30", "2025-10-26"),
            (2026, "2026-03-08", "2026-11-01", "2026-03-29", "2026-10-25"),
            (2027, "2027-03-14", "2027-11-07", "2027-03-28", "2027-10-31"),
            (2028, "2028-03-12", "2028-11-05", "2028-03-26", "2028-10-29"),
        ] {
            assert_eq!(us.switch_dates(year), (d(us_on), d(us_off)), "{year}");
            assert_eq!(eu.switch_dates(year), (d(eu_on), d(eu_off)), "{year}");
        }
        // Ahead from the day the clocks go forward, not on the day they go back.
        assert!(!us.in_effect(d("2026-03-07")));
        assert!(us.in_effect(d("2026-03-08")));
        assert!(us.in_effect(d("2026-10-31")));
        assert!(!us.in_effect(d("2026-11-01")));
        assert!(!eu.in_effect(d("2026-01-15")) && eu.in_effect(d("2026-07-15")));
        assert!(!eu.in_effect(d("2026-10-25")));
        // The southern rules cross the new year: on from the spring switch to the autumn one.
        let au = DaylightSaving::Australia;
        let nz = DaylightSaving::NewZealand;
        for (year, au_on, au_off, nz_on, nz_off) in [
            (2024, "2024-10-06", "2024-04-07", "2024-09-29", "2024-04-07"),
            (2025, "2025-10-05", "2025-04-06", "2025-09-28", "2025-04-06"),
            (2026, "2026-10-04", "2026-04-05", "2026-09-27", "2026-04-05"),
            (2027, "2027-10-03", "2027-04-04", "2027-09-26", "2027-04-04"),
        ] {
            assert_eq!(au.switch_dates(year), (d(au_on), d(au_off)), "{year}");
            assert_eq!(nz.switch_dates(year), (d(nz_on), d(nz_off)), "{year}");
        }
        // Ahead over the southern summer, back in the middle of the year.
        assert!(au.in_effect(d("2026-01-15")) && au.in_effect(d("2026-10-04")));
        assert!(!au.in_effect(d("2026-04-05")) && !au.in_effect(d("2026-07-15")));
        assert!(nz.in_effect(d("2026-01-15")) && nz.in_effect(d("2026-09-27")));
        assert!(!nz.in_effect(d("2026-04-05")) && !nz.in_effect(d("2026-07-15")));
        assert_eq!(DaylightSaving::from_name("Australia"), Some(au));
        assert_eq!(DaylightSaving::from_name("new_zealand"), Some(nz));
        assert_eq!(DaylightSaving::from_name(" EU "), Some(eu));
        assert_eq!(DaylightSaving::from_name("us_canada"), Some(us));
        assert_eq!(DaylightSaving::from_name("none"), None);
    }

    /// BYD-08: Israel, Egypt and mainland Chile, from published calendars.
    #[test]
    fn more_daylight_saving_rules() {
        let (il, eg, cl) = (
            DaylightSaving::Israel,
            DaylightSaving::Egypt,
            DaylightSaving::Chile,
        );
        for (year, il_on, il_off, eg_on, eg_off, cl_on, cl_off) in [
            (
                2024,
                "2024-03-29",
                "2024-10-27",
                "2024-04-26",
                "2024-11-01",
                "2024-09-08",
                "2024-04-07",
            ),
            (
                2025,
                "2025-03-28",
                "2025-10-26",
                "2025-04-25",
                "2025-10-31",
                "2025-09-07",
                "2025-04-06",
            ),
            (
                2026,
                "2026-03-27",
                "2026-10-25",
                "2026-04-24",
                "2026-10-30",
                "2026-09-06",
                "2026-04-05",
            ),
            (
                2027,
                "2027-03-26",
                "2027-10-31",
                "2027-04-30",
                "2027-10-29",
                "2027-09-05",
                "2027-04-04",
            ),
        ] {
            assert_eq!(
                il.switch_dates(year),
                (d(il_on), d(il_off)),
                "Israel {year}"
            );
            assert_eq!(eg.switch_dates(year), (d(eg_on), d(eg_off)), "Egypt {year}");
            assert_eq!(cl.switch_dates(year), (d(cl_on), d(cl_off)), "Chile {year}");
        }
        assert!(il.in_effect(d("2026-03-27")) && !il.in_effect(d("2026-03-26")));
        assert!(!il.in_effect(d("2026-10-25")));
        // Egypt: the last Thursday of October is still summer time, the Friday isn't.
        assert!(eg.in_effect(d("2026-10-29")) && !eg.in_effect(d("2026-10-30")));
        assert!(eg.in_effect(d("2026-04-24")) && !eg.in_effect(d("2026-04-23")));
        // Chile is southern: ahead over its summer, across the new year.
        assert!(cl.in_effect(d("2026-01-15")) && cl.in_effect(d("2026-09-06")));
        assert!(!cl.in_effect(d("2026-04-05")) && !cl.in_effect(d("2026-07-15")));
        for r in DaylightSaving::ALL {
            assert_eq!(DaylightSaving::from_name(r.name()), Some(r));
        }
        // The last weekday falls on the month's last days too.
        assert_eq!(last_weekday(2026, 4, FRIDAY), d("2026-04-24"));
        assert_eq!(last_weekday(2027, 4, FRIDAY), d("2027-04-30"));
        assert_eq!(last_weekday(2025, 10, THURSDAY), d("2025-10-30"));
    }

    /// Decision 193: sunrise jumps an hour (less the day's two minutes or so) across the
    /// day the clocks go forward.
    #[test]
    fn sunrise_across_a_switch() {
        let rise = |s: Sun| match s {
            Sun::Times { rise, .. } => rise,
            other => panic!("{other:?}"),
        };
        let offset = |rule: DaylightSaving, date: Date, standard: i32| {
            standard + if rule.in_effect(date) { 60 } else { 0 }
        };
        // New York, 2024: EST on the 9th of March, about 06:17, then EDT on the 10th, an
        // hour later on the clock less the day's minute and a half of earlier sun.
        let us = DaylightSaving::UsCanada;
        let (sat, sun_day) = (d("2024-03-09"), d("2024-03-10"));
        let a = rise(sun(sat, 40_712_800, -74_006_000, offset(us, sat, -300)));
        let b = rise(sun(
            sun_day,
            40_712_800,
            -74_006_000,
            offset(us, sun_day, -300),
        ));
        assert!((6 * 60 + 14..=6 * 60 + 20).contains(&a), "{a}");
        assert!((57..=59).contains(&(b - a)), "{a} then {b}");
        // London, 2024: GMT on the 30th of March, BST on the 31st.
        let eu = DaylightSaving::Eu;
        let (sat, sun_day) = (d("2024-03-30"), d("2024-03-31"));
        let a = rise(sun(sat, 51_507_400, -127_800, offset(eu, sat, 0)));
        let b = rise(sun(sun_day, 51_507_400, -127_800, offset(eu, sun_day, 0)));
        assert!((55..=59).contains(&(b - a)), "{a} then {b}");
        // And back an hour across the first Sunday of November.
        let (sat, sun_day) = (d("2024-11-02"), d("2024-11-03"));
        let a = rise(sun(sat, 40_712_800, -74_006_000, offset(us, sat, -300)));
        let b = rise(sun(
            sun_day,
            40_712_800,
            -74_006_000,
            offset(us, sun_day, -300),
        ));
        assert!((-60..=-58).contains(&(b - a)), "{a} then {b}");
    }

    #[test]
    fn polar_days_and_nights() {
        // Tromso has the midnight sun at midsummer and polar night at midwinter.
        assert_eq!(
            sun(d("2024-06-21"), 69_649_200, 18_955_300, 120),
            Sun::AlwaysUp
        );
        assert_eq!(
            sun(d("2024-12-21"), 69_649_200, 18_955_300, 60),
            Sun::AlwaysDown
        );
        assert_eq!(sun(d("2024-06-21"), 90_000_000, 0, 0), Sun::AlwaysUp);
        assert_eq!(sun(d("2024-06-21"), -90_000_000, 0, 0), Sun::AlwaysDown);
    }
}
