//! Regional climate predictions from actual MCD 6.2 exports, not invented observations.
use super::MarsClock;
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
pub const SOURCE: &str = "https://www-mars.lmd.jussieu.fr/mcd_python/";
#[derive(Deserialize)]
pub struct ClimateModel {
    sites: BTreeMap<String, Region>,
}
#[derive(Deserialize)]
struct Region {
    grids: BTreeMap<String, Grid>,
}
#[derive(Deserialize)]
struct Grid {
    values: Vec<Vec<f64>>,
}
#[derive(Clone, Debug)]
pub struct Prediction {
    pub value: f64,
    pub unit: &'static str,
    pub label: &'static str,
}
impl ClimateModel {
    pub fn bundled() -> Result<Self> {
        let model: Self =
            serde_json::from_str(include_str!("../../assets/data/climate_model.json"))?;
        for id in ["jezero", "gale"] {
            let r = model
                .sites
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Missing climate region {id}"))?;
            for key in [
                "t",
                "ps",
                "u",
                "v",
                "tsurf",
                "rho",
                "tauref",
                "fluxsurf_dn_sw",
                "dust_dep",
                "thermal_inertia",
                "ground_albedo",
                "col_h2ovapor",
                "vmr_h2o",
                "vmr_co2",
                "pscaleheight",
            ] {
                let g = r
                    .grids
                    .get(key)
                    .ok_or_else(|| anyhow::anyhow!("Missing model variable {key}"))?;
                ensure!(
                    g.values.len() == 24
                        && g.values
                            .iter()
                            .all(|r| r.len() == 24 && r.iter().all(|x| x.is_finite())),
                    "Invalid climate grid"
                );
            }
        }
        Ok(model)
    }
    pub fn sample(&self, site: &str, key: &str, ls: f64, local_true_hours: f64) -> Option<f64> {
        if !ls.is_finite() || !local_true_hours.is_finite() {
            return None;
        }
        let g = &self.sites.get(site)?.grids.get(key)?.values;
        let x = ls.rem_euclid(360.) / 15.;
        let y = local_true_hours.rem_euclid(24.);
        let i = x.floor() as usize;
        let j = y.floor() as usize;
        let a = x.fract();
        let b = y.fract();
        let lo = g[i][j] * (1. - b) + g[i][(j + 1) % 24] * b;
        let hi = g[(i + 1) % 24][j] * (1. - b) + g[(i + 1) % 24][(j + 1) % 24] * b;
        Some(lo * (1. - a) + hi * a)
    }
    pub fn predict(&self, site: &str, ls: f64, hour: f64) -> BTreeMap<&'static str, Prediction> {
        let mut out = BTreeMap::new();
        let s = |key| self.sample(site, key, ls, hour);
        let mut add = |key, value: Option<f64>, unit, label| {
            if let Some(value) = value.filter(|v| v.is_finite()) {
                out.insert(key, Prediction { value, unit, label });
            }
        };
        add(
            "air_temperature",
            s("t").map(|v| v - 273.15),
            "°C",
            "Air temperature",
        );
        add(
            "surface_temperature",
            s("tsurf").map(|v| v - 273.15),
            "°C",
            "Ground temperature",
        );
        add("pressure", s("ps"), "Pa", "Pressure");
        if let (Some(u), Some(v)) = (s("u"), s("v")) {
            add("wind_speed", Some(u.hypot(v)), "m/s", "Wind speed");
            if u.hypot(v) > 0.01 {
                add(
                    "wind_direction",
                    Some((-u).atan2(-v).to_degrees().rem_euclid(360.)),
                    "° azimuth",
                    "Wind from",
                );
            }
        }
        for (key, var, unit, label, multiplier) in [
            ("density", "rho", "kg/m³", "Atmospheric density", 1.),
            ("dust_tau", "tauref", "τ", "Dust optical depth", 1.),
            (
                "solar_irradiance",
                "fluxsurf_dn_sw",
                "W/m²",
                "Solar irradiance",
                1.,
            ),
            (
                "dust_deposition",
                "dust_dep",
                "mg/cm²/sol",
                "Dust deposition",
                MarsClock::SECONDS_PER_SOL * 100.,
            ),
            (
                "thermal_inertia",
                "thermal_inertia",
                "J m⁻² K⁻¹ s⁻½",
                "Thermal inertia",
                1.,
            ),
            ("albedo", "ground_albedo", "0–1", "Surface albedo", 1.),
            (
                "water_column",
                "col_h2ovapor",
                "precipitable µm",
                "Water vapor column",
                1000.,
            ),
        ] {
            add(key, s(var).map(|v| v * multiplier), unit, label);
        }
        add(
            "scale_height",
            s("pscaleheight").map(|m| m / 1000.),
            "km",
            "Atmospheric scale height",
        );
        add(
            "gas_mixing",
            s("vmr_co2").map(|v| v * 100.),
            "% CO₂ by volume",
            "CO₂ volume mixing ratio",
        );
        if let (Some(t), Some(p), Some(q), Some(h)) =
            (s("t"), s("ps"), s("vmr_h2o"), s("pscaleheight"))
            && (110.0..=273.16).contains(&t)
            && h > 0.
        {
            // Murphy & Koop 2005, Eq. 7, DOI 10.1256/qj.04.94.
            // Near-surface vapor pressure uses the model's water-vapor VMR.
            let saturation = (9.550426 - 5723.265 / t + 3.53068 * t.ln() - 0.00728332 * t).exp();
            add(
                "humidity",
                Some(q * p * (-1.5 / h).exp() / saturation * 100.),
                "% RH over ice",
                "Relative humidity over ice",
            );
        }
        let earth_hours_per_mars_hour = MarsClock::SECONDS_PER_SOL / 24. / 3600.;
        add(
            "thermal_rate",
            self.sample(site, "t", ls, hour + 0.5)
                .zip(self.sample(site, "t", ls, hour - 0.5))
                .map(|(a, b)| (a - b) / earth_hours_per_mars_hour),
            "°C/h",
            "Thermal change rate",
        );
        for (var, lo, hi) in [
            ("t", "air_min", "air_max"),
            ("tsurf", "surface_min", "surface_max"),
        ] {
            let samples = (0..24)
                .filter_map(|h| self.sample(site, var, ls, h as f64))
                .collect::<Vec<_>>();
            if !samples.is_empty() {
                add(
                    lo,
                    Some(samples.iter().copied().fold(f64::INFINITY, f64::min) - 273.15),
                    "°C",
                    "Daily low",
                );
                add(
                    hi,
                    Some(samples.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 273.15),
                    "°C",
                    "Daily high",
                );
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::ClimateModel;
    #[test]
    fn exported_model_nodes_and_periodic_boundaries_are_preserved() {
        let m = ClimateModel::bundled().unwrap();
        assert!((m.sample("jezero", "t", 0., 12.).unwrap() - 235.778).abs() < 0.001);
        for id in ["jezero", "gale"] {
            assert_eq!(m.sample(id, "t", 0., 0.), m.sample(id, "t", 360., 24.));
        }
    }
    #[test]
    fn predictions_remain_distinct_from_unsupported_geology_and_operations() {
        let m = ClimateModel::bundled().unwrap();
        for id in ["jezero", "gale"] {
            let p = m.predict(id, 90., 12.);
            assert!(p["pressure"].value > 0.);
            assert!(p["air_temperature"].value > -150. && p["air_temperature"].value < 60.);
            assert!(
                !p.contains_key("radiation")
                    && !p.contains_key("caves")
                    && !p.contains_key("comms")
            );
            assert!(p["air_min"].value <= p["air_max"].value);
        }
        assert!(m.sample("unknown", "t", 0., 0.).is_none());
        assert!(m.sample("jezero", "t", f64::NAN, 0.).is_none());
    }
}
