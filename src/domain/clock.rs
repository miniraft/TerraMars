use chrono::{DateTime, Datelike, Utc};

/// NASA GISS Mars24, using the revised 2025 equations and historical TAI−UTC.
/// https://www.giss.nasa.gov/tools/mars24/help/algorithm.html
pub struct MarsClock;

impl MarsClock {
    pub const SECONDS_PER_SOL: f64 = 88775.244147;
    pub const SOURCE_URL: &'static str =
        "https://www.giss.nasa.gov/tools/mars24/help/algorithm.html";

    pub fn at(utc: DateTime<Utc>, longitude_east: f64, latitude: f64) -> MarsTime {
        let jd = 2440587.5 + utc.timestamp_micros() as f64 / 86_400_000_000.0;
        let centuries = (jd - 2451545.0) / 36525.0;
        let offset = if utc.year() < 1972 {
            64.184 + 59.0 * centuries
                - 51.2 * centuries.powi(2)
                - 67.1 * centuries.powi(3)
                - 16.4 * centuries.powi(4)
        } else {
            32.184 + tai_minus_utc(utc)
        };
        let jd_tt = jd + offset / 86400.0;
        let d = jd_tt - 2451545.0;
        let anomaly = 19.3871 + 0.52402073 * d;
        let fms = 270.3871 + 0.524038496 * d;
        let perturbations = [
            (0.0071, 2.2353, 49.409),
            (0.0057, 2.7543, 168.173),
            (0.0039, 1.1177, 191.837),
            (0.0037, 15.7866, 21.736),
            (0.0021, 2.1354, 15.704),
            (0.0020, 2.4694, 95.528),
            (0.0018, 32.8493, 49.095),
        ];
        let pbs: f64 = perturbations
            .iter()
            .map(|(a, period, phase)| a * cos(0.985626 * d / period + phase))
            .sum();
        let center = (10.691 + 3e-7 * d) * sin(anomaly)
            + 0.623 * sin(2.0 * anomaly)
            + 0.050 * sin(3.0 * anomaly)
            + 0.005 * sin(4.0 * anomaly)
            + 0.0005 * sin(5.0 * anomaly)
            + pbs;
        let raw_ls = fms + center;
        let ls = raw_ls.rem_euclid(360.0);
        let eot = 2.861 * sin(2.0 * ls) - 0.071 * sin(4.0 * ls) + 0.002 * sin(6.0 * ls) - center;
        let msd = (jd_tt - 2451549.5) / 1.0274912517 + 44796.0 - 0.0009626;
        let airy = (24.0 * msd).rem_euclid(24.0);
        let lmst = (airy + longitude_east / 15.0).rem_euclid(24.0);
        let ltst = (lmst + eot / 15.0).rem_euclid(24.0);
        let declination = (0.42565 * sin(ls)).asin().to_degrees() + 0.25 * sin(ls);
        let hour_angle = (ltst - 12.0) * 15.0;
        // NASA's map latitude is planetocentric; the solar elevation formula
        // takes planetographic latitude. Reference radii are in km.
        let graphic_latitude = (latitude.to_radians().tan() * (3396.2_f64 / 3376.2).powi(2))
            .atan()
            .to_degrees();
        let solar_altitude = (sin(graphic_latitude) * sin(declination)
            + cos(graphic_latitude) * cos(declination) * cos(hour_angle))
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees();
        let au = 1.52367934
            * (1.00436
                - 0.09309 * cos(anomaly)
                - 0.004336 * cos(2.0 * anomaly)
                - 0.00031 * cos(3.0 * anomaly)
                - 0.00003 * cos(4.0 * anomaly));
        MarsTime {
            utc,
            msd,
            airy_hours: airy,
            local_mean_hours: lmst,
            local_true_hours: ltst,
            solar_longitude: ls,
            solar_declination: declination,
            solar_altitude,
            heliocentric_distance_au: au,
            // MY1's northern spring equinox (1955) is raw Ls −8280°.
            mars_year: ((raw_ls + 8280.0) / 360.0).floor() as i32 + 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarsTime {
    pub utc: DateTime<Utc>,
    pub msd: f64,
    pub airy_hours: f64,
    pub local_mean_hours: f64,
    pub local_true_hours: f64,
    pub solar_longitude: f64,
    pub solar_declination: f64,
    pub solar_altitude: f64,
    pub heliocentric_distance_au: f64,
    pub mars_year: i32,
}

impl MarsTime {
    pub fn sol(&self) -> i64 {
        self.msd.floor() as i64
    }
    pub fn is_daylight(&self) -> bool {
        self.solar_altitude > 0.0
    }
    pub fn formatted_local_time(&self) -> String {
        format_hours(self.local_mean_hours)
    }
    pub fn formatted_airy_time(&self) -> String {
        format_hours(self.airy_hours)
    }
    pub fn date_label(&self) -> String {
        format!("MY {} · MSD {}", self.mars_year, self.sol())
    }
    pub fn season(&self) -> &'static str {
        [
            "Northern spring",
            "Northern summer",
            "Northern autumn",
            "Northern winter",
        ][((self.solar_longitude / 90.0).floor() as usize).min(3)]
    }

    /// Geometric flat-ground estimate, excluding terrain and refraction.
    pub fn shadow_length(&self, object_height_meters: f64) -> Option<f64> {
        if !object_height_meters.is_finite()
            || object_height_meters <= 0.0
            || self.solar_altitude <= 0.0
        {
            None
        } else {
            Some(object_height_meters / self.solar_altitude.to_radians().tan())
        }
    }
}

pub fn format_hours(hours: f64) -> String {
    let seconds = ((hours * 3600.0).floor() as i64).rem_euclid(86400);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

fn sin(degrees: f64) -> f64 {
    degrees.to_radians().sin()
}
fn cos(degrees: f64) -> f64 {
    degrees.to_radians().cos()
}

fn tai_minus_utc(utc: DateTime<Utc>) -> f64 {
    let epochs = [
        (1972, 1, 10),
        (1972, 7, 11),
        (1973, 1, 12),
        (1974, 1, 13),
        (1975, 1, 14),
        (1976, 1, 15),
        (1977, 1, 16),
        (1978, 1, 17),
        (1979, 1, 18),
        (1980, 1, 19),
        (1981, 7, 20),
        (1982, 7, 21),
        (1983, 7, 22),
        (1985, 7, 23),
        (1988, 1, 24),
        (1990, 1, 25),
        (1991, 1, 26),
        (1992, 7, 27),
        (1993, 7, 28),
        (1994, 7, 29),
        (1996, 1, 30),
        (1997, 7, 31),
        (1999, 1, 32),
        (2006, 1, 33),
        (2009, 1, 34),
        (2012, 7, 35),
        (2015, 7, 36),
        (2017, 1, 37),
    ];
    let current = (utc.year(), utc.month());
    epochs
        .iter()
        .rev()
        .find(|(year, month, _)| current >= (*year, *month))
        .map_or(10.0, |(_, _, offset)| f64::from(*offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn matches_nasa_mars24_worked_example() {
        let time = MarsClock::at(Utc.with_ymd_and_hms(2000, 1, 6, 0, 0, 0).unwrap(), 0.0, 0.0);
        assert!((time.airy_hours - 23.99425).abs() < 0.00001);
        assert!((time.solar_longitude - 277.18758).abs() < 0.0001);
        assert_eq!(time.formatted_airy_time(), "23:59:39");
        assert_eq!(time.mars_year, 24);
    }

    #[test]
    fn east_longitude_advances_local_time_and_a_physical_sol_advances_msd() {
        let now = Utc.with_ymd_and_hms(2026, 11, 14, 0, 0, 0).unwrap();
        let prime = MarsClock::at(now, 0.0, 0.0);
        let east = MarsClock::at(now, 90.0, 0.0);
        assert!(
            ((east.local_mean_hours - prime.local_mean_hours).rem_euclid(24.0) - 6.0).abs() < 1e-8
        );
        let next = MarsClock::at(
            now + Duration::microseconds((MarsClock::SECONDS_PER_SOL * 1e6).round() as i64),
            0.0,
            0.0,
        );
        assert!((next.msd - prime.msd - 1.0).abs() < 1e-8);
    }
}
