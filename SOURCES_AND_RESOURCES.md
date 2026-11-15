# TerraMars sources and resources

This list is based on TerraMars’s code, asset manifests, and documentation. It covers scientific sources, assets, references, and direct software dependencies; transitive software dependencies are recorded in `Cargo.lock`.

## Maps and terrain

- [NASA Trek CTX imagery metadata](https://trek.nasa.gov/tiles/Mars/EQ/CTX_beta01_uncontrolled_5m_Caltech/1.0.0/WMTSCapabilities.xml) — orbital imagery for Jezero and Gale.
- [Caltech/Murray Lab CTX tiles via Esri OnMars](https://astro.arcgis.com/arcgis/rest/services/OnMars/CTX/MapServer) — source map tiles. Credits: NASA/JPL/MSSS/Caltech Murray Lab/Esri.
- [NASA Trek MOLA shaded relief](https://trek.nasa.gov/tiles/Mars/EQ/Mars_MGS_MOLA_ClrShade_merge_global_463m/1.0.0/WMTSCapabilities.xml) — colored terrain layer.
- [NASA PDS MOLA MEGDR elevation archive](https://pds-geosciences.wustl.edu/missions/mgs/megdr.html) — numerical elevations used to calculate slopes and screen routes.
- [NASA Trek API documentation](https://trek.nasa.gov/tiles/apidoc/trekAPI.html?body=mars).

## Rover positions and recorded routes

- Perseverance: [latest published position](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_waypoints_current.json), [waypoints](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_waypoints.json), [traverse](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_traverse.json).
- Curiosity: [latest published position](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_waypoints_current.json), [waypoints](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_waypoints.json), [traverse](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_traverse.json).

## Weather, climate, and radiation

- [Perseverance/MEDA NASA weather feed](https://mars.nasa.gov/rss/api/?feed=weather&category=mars2020&feedtype=json) — historical Jezero observations.
- [Curiosity/REMS NASA weather feed](https://mars.nasa.gov/rss/api/?feed=weather&category=msl&feedtype=json) — historical Gale observations.
- [Mars Climate Database 6.2](https://www-mars.lmd.jussieu.fr/mcd_python/) — regional modeled climate values; credited to LMD/OU/IAA/ESA/CNES.
- [Murphy & Koop (2005)](https://doi.org/10.1256/qj.04.94) — ice saturation vapor-pressure equation used for modeled relative humidity.
- [NASA Curiosity radiation results](https://www.nasa.gov/news-release/nasa-rover-results-include-first-age-measurement-on-mars-and-help-for-human-exploration/) — historical Gale radiation reference.
- [Calibrated MEDA products](https://pds-atmospheres.nmsu.edu/data_and_services/atmospheres_data/PERSEVERANCE/meda.html) — linked scientific resource for future integration, rather than the bundled weather source.

Weather observations retain their historical dates. Climate predictions are model outputs, and the radiation reference is a historical period average.

## Geographic names and calculation references

- IAU/USGS Gazetteer: [Jezero](https://planetarynames.wr.usgs.gov/Feature/14300), [Gale](https://planetarynames.wr.usgs.gov/Feature/2071), [Neretva Vallis](https://planetarynames.wr.usgs.gov/Feature/15879), [Aeolis Mons](https://planetarynames.wr.usgs.gov/Feature/15000).
- [NASA Mars24 algorithms](https://www.giss.nasa.gov/tools/mars24/help/algorithm.html) and [technical notes](https://www.giss.nasa.gov/tools/mars24/help/notes.html) — Mars time and solar calculations.
- [NASA Mars fact sheet](https://nssdc.gsfc.nasa.gov/planetary/factsheet/marsfact.html) — planetary dimensions used in geographic calculations.

## Images and fonts

- [Curiosity’s 1.8-billion-pixel panorama, PIA23623](https://science.nasa.gov/photojournal/curiositys-18-billion-pixel-panorama/) — current decorative background. Credit: NASA/JPL-Caltech/MSSS.
- [Mastcam-Z’s first 360° panorama, PIA24264](https://www.jpl.nasa.gov/images/pia24264-mastcam-zs-first-360-degree-panorama/) — earlier background documented in the project.
- [Inter](https://github.com/rsms/inter) — body font.
- [Manrope](https://github.com/googlefonts/manrope) — heading font.

## Software and development resources

- [Rust](https://www.rust-lang.org/) and [rustup](https://rustup.rs/) — application language and toolchain.
- [GPUI Community Edition](https://github.com/gpui-ce/gpui-ce) — desktop interface and platform backends.
- Direct Rust libraries: [anyhow](https://crates.io/crates/anyhow), [chrono](https://crates.io/crates/chrono), [directories](https://crates.io/crates/directories), [image](https://crates.io/crates/image), [reqwest](https://crates.io/crates/reqwest), [serde](https://crates.io/crates/serde), [serde_json](https://crates.io/crates/serde_json), [env_logger](https://crates.io/crates/env_logger), [log](https://crates.io/crates/log), and [embed-resource](https://crates.io/crates/embed-resource).
- [Python](https://www.python.org/) and [Pillow](https://pillow.readthedocs.io/) — data preparation, packaging, and image processing.
- Retained voice utilities: [Vosk](https://alphacephei.com/vosk/), [Vosk models](https://alphacephei.com/vosk/models), and [sounddevice](https://python-sounddevice.readthedocs.io/). Voice input is removed from the current app.

## Project provenance records

- [Scientific data sources](docs/DATA_SOURCES.md).
- [Climate model documentation](docs/CLIMATE_MODEL.md).
- [Terrain data and routing](docs/TERRAIN_ROUTING.md).
- [Feature requirements and calculation references](docs/FEATURE_MATRIX.md).
- [Original source URLs and checksums](assets/data/source_manifest.json).
- [Map sources, bounds, credits, and checksums](assets/maps/manifest.json).
- [Current panorama provenance](assets/maps/panorama_source.json).
- [Rust dependencies](Cargo.toml).
- [Voice utility dependencies](tools/voice-requirements.txt).

The user-provided **Space Apps 2026.docx** supplied feature requirements and research leads. No public link is recorded. Its statements are not treated as scientific measurements.
