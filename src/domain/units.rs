use serde::{Deserialize, Serialize};

/// Both unit sets are SI: Celsius/Kelvin, km/m, hPa/Pa, km/h/m/s.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystem {
    #[default]
    Metric,
    Scientific,
}

impl UnitSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metric => "Metric",
            Self::Scientific => "Scientific",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Metric => Self::Scientific,
            Self::Scientific => Self::Metric,
        }
    }

    pub fn temperature_c(self, celsius: Option<f64>) -> String {
        match celsius.filter(|value| value.is_finite()) {
            None => "Unavailable".into(),
            Some(value) => match self {
                Self::Metric => format!("{value:.1} °C"),
                Self::Scientific => format!("{:.1} K", value + 273.15),
            },
        }
    }

    pub fn distance_km(self, km: f64) -> String {
        match self {
            Self::Metric => format!("{km:.2} km"),
            Self::Scientific => format!("{:.0} m", km * 1000.0),
        }
    }

    pub fn elevation_meters(self, meters: f64) -> String {
        match self {
            Self::Metric => format!("{:.3} km", meters / 1000.0),
            Self::Scientific => format!("{meters:.0} m"),
        }
    }

    pub fn speed_kmh(self, kmh: f64) -> String {
        match self {
            Self::Metric => format!("{kmh:.1} km/h"),
            Self::Scientific => format!("{:.2} m/s", kmh / 3.6),
        }
    }

    pub fn wind_mps(self, mps: Option<f64>) -> String {
        match mps.filter(|value| value.is_finite()) {
            None => "Unavailable".into(),
            Some(value) => match self {
                Self::Metric => format!("{:.1} km/h", value * 3.6),
                Self::Scientific => format!("{value:.1} m/s"),
            },
        }
    }

    pub fn pressure_pa(self, pa: Option<f64>) -> String {
        match pa.filter(|value| value.is_finite()) {
            None => "Unavailable".into(),
            Some(value) => match self {
                Self::Metric => format!("{:.2} hPa", value / 100.0),
                Self::Scientific => format!("{value:.1} Pa"),
            },
        }
    }

    /// Formats only the measurements whose source unit supports conversion.
    pub fn observation(self, value: Option<f64>, unit: &str) -> String {
        let Some(value) = value.filter(|value| value.is_finite()) else {
            return "Unavailable".into();
        };
        match unit {
            "°C" | "C" => self.temperature_c(Some(value)),
            "K" => self.temperature_c(Some(value - 273.15)),
            "Pa" => self.pressure_pa(Some(value)),
            "hPa" => self.pressure_pa(Some(value * 100.0)),
            "m/s" => self.wind_mps(Some(value)),
            "m" => self.elevation_meters(value),
            "km" => self.distance_km(value),
            "kg/m³" => format!("{value:.4} {unit}"),
            _ if value != 0. && value.abs() < 0.01 => format!("{value:.3e} {unit}"),
            _ => format!("{value:.2} {unit}"),
        }
    }

    pub fn duration(seconds: u64) -> String {
        let minutes = seconds.saturating_add(59) / 60;
        if minutes < 60 {
            format!("{minutes} min")
        } else if minutes.is_multiple_of(60) {
            format!("{} h", minutes / 60)
        } else {
            format!("{} h {} min", minutes / 60, minutes % 60)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions_preserve_missing_values_and_original_physical_units() {
        assert_eq!(UnitSystem::Metric.temperature_c(Some(-60.0)), "-60.0 °C");
        assert!(
            UnitSystem::Scientific
                .temperature_c(Some(-60.0))
                .starts_with("213.")
        );
        assert_eq!(UnitSystem::Scientific.distance_km(1.0), "1000 m");
        assert_eq!(UnitSystem::Metric.pressure_pa(Some(700.0)), "7.00 hPa");
        assert_eq!(UnitSystem::Scientific.pressure_pa(Some(700.0)), "700.0 Pa");
        assert_eq!(UnitSystem::Metric.temperature_c(None), "Unavailable");
        assert_eq!(
            UnitSystem::Metric.temperature_c(Some(f64::NAN)),
            "Unavailable"
        );
        assert_eq!(UnitSystem::duration(3661), "1 h 2 min");
    }
}
