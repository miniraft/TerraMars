# TerraMars scientific data and provenance

The bundled mission data were retrieved on **2026-11-14 UTC** from the sources below. Retrieval date is not observation date. The attached research document supplied feature requirements and research leads; its statements are not treated as measurements or operating instructions. All numeric map, environmental, rover, and named-feature values in the catalog have a separate primary source. No current astronaut mission, measured astronaut position, local forecast, or terrain-safety certification is implied.

## Maps: actual orbital observations

Both maps are actual Mars Reconnaissance Orbiter Context Camera (CTX) orbital mosaics. These are grayscale orbital photographs, not generated scenery, Earth maps, or illustrations. NASA Trek's CTX WMTS metadata points to the Caltech/Murray Lab global mosaic distributed through the Esri OnMars service. The full-resolution local JPEGs are assembled from those original source tiles, cropped to an exact geographic rectangle. Both maps are bundled and available offline; the app does not stream map tiles.

- [NASA Trek CTX capabilities](https://trek.nasa.gov/tiles/Mars/EQ/CTX_beta01_uncontrolled_5m_Caltech/1.0.0/WMTSCapabilities.xml)
- [CTX source tile service](https://astro.arcgis.com/arcgis/rest/services/OnMars/CTX/MapServer)
- [NASA Trek MOLA color shaded relief capabilities](https://trek.nasa.gov/tiles/Mars/EQ/Mars_MGS_MOLA_ClrShade_merge_global_463m/1.0.0/WMTSCapabilities.xml)
- [NASA Trek API documentation](https://trek.nasa.gov/tiles/apidoc/trekAPI.html?body=mars)

Credits: CTX **NASA/JPL/MSSS/Caltech Murray Lab/Esri**; MOLA **NASA/JPL/GSFC**. CTX is the preliminary *uncontrolled* mosaic; small registration errors are possible. MOLA is a genuine color shaded-relief visualization, **not a numeric DEM**. The app does not derive numeric elevations, slopes, or obstacles from its colors.

| Asset | Pixels | West | East | South | North |
| --- | --- | --- | --- | --- | --- |
| `jezero_ctx.jpg` | 3496 × 3496 | 76.94995880126953 | 78.15021514892578 | 17.899818420410156 | 19.100074768066406 |
| `gale_ctx.jpg` | 4370 × 4370 | 136.29981994628906 | 139.3004608154297 | −6.9001007080078125 | −3.8994598388671875 |

The corresponding `*_mola.jpg` layers share the exact bounds above, resampled from source tiles. Their lower resolution is visible when zoomed in. Image projection is **Mars equirectangular, planetocentric latitude, east-positive longitude**. Geographic overlay positions are linear in longitude/latitude. Earth Web Mercator and Earth's radius must not be used. Source tile levels, pixels, URLs, retrieval dates, notes, and SHA-256 checksums are preserved in `assets/maps/manifest.json`.

## Rover locations, traverses, and landmarks

- [Perseverance current published waypoint](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_waypoints_current.json)
- [Perseverance waypoints](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_waypoints.json)
- [Perseverance traverse](https://mars.nasa.gov/mmgis-maps/M20/Layers/json/M20_traverse.json)
- [Curiosity current published waypoint](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_waypoints_current.json)
- [Curiosity waypoints](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_waypoints.json)
- [Curiosity traverse](https://mars.nasa.gov/mmgis-maps/MSL/Layers/json/MSL_traverse.json)

| Bundled latest published localization | Mission sol | Latitude | Longitude | Geoid-relative elevation |
| --- | --- | --- | --- | --- |
| Perseverance | 2022 | 18.43559434 | 77.23322287 | −1936.366943 m |
| Curiosity | 5073 | −4.82602331 | 137.388886 | −3472.085 m |

The feeds provide mission sols, not UTC observation timestamps. The app preserves the sol instead of claiming that the retrieval date is the measurement date. The raw current-point GeoJSONs are included. Historical route geometry has 36,661 source vertices for Perseverance and 27,018 for Curiosity. Per-source-segment Douglas–Peucker simplification at approximately 2 m tolerance retains 2,010 and 2,546 original vertices respectively. No artificial route vertices are introduced, and each original segment's endpoints are retained. A historical rover path is evidence of past rover travel, **not validation of a present astronaut path**. Sand, steep terrain, changing obstacles, and EVA life-support budgets need separate validation.

Science-waypoint markers use actual NASA waypoint coordinates and are named by their source sol. They are not invented geological discoveries. The landing-vicinity markers are early recorded rover waypoints, explicitly distinguished from exact touchdown coordinates. Rover chassis tilt is attitude telemetry, **not terrain slope**.

Regional named-feature centers come from the IAU/USGS Gazetteer:

- [Jezero, feature 14300](https://planetarynames.wr.usgs.gov/Feature/14300): center 18.41° N, 77.69° E; diameter 47.52 km.
- [Gale, feature 2071](https://planetarynames.wr.usgs.gov/Feature/2071): center −5.44°, 137.70° E; diameter 155.30 km.
- [Neretva Vallis, feature 15879](https://planetarynames.wr.usgs.gov/Feature/15879): center 18.55° N, 77.20° E.
- [Aeolis Mons, feature 15000](https://planetarynames.wr.usgs.gov/Feature/15000): center −5.08°, 137.85° E.

A gazetteer center is a regional label; it is not a surveyed stopping point, mountain summit, or safe approach. The research document's Gale coordinate near −4.59°, 137.44° E identifies the landing region, not the crater center. NASA popular references round Jezero/Gale diameters to about 45/154 km; this catalog uses the current Gazetteer values above.

## Environmental observations: historical, local to instruments

### Jezero — Perseverance MEDA

[Official NASA public weather feed](https://mars.nasa.gov/rss/api/?feed=weather&category=mars2020&feedtype=json), retrieved 2026-11-14, contains seven records dated 2024-04-21 through 2024-04-27. The newest record is **2024-04-27 / sol 1133**:

- Daily low air temperature: **−79.3 °C**.
- Daily high air temperature: **−24.7 °C**.
- Surface pressure: **778.9 Pa**.

The app labels these as historical. Missing ground-temperature, humidity, and wind fields are unavailable. No value from Curiosity or InSight is substituted for Jezero. [Calibrated MEDA science products](https://pds-atmospheres.nmsu.edu/data_and_services/atmospheres_data/PERSEVERANCE/meda.html) are linked for subsequent scientific integration.

### Gale — Curiosity REMS

[Official NASA public weather feed](https://mars.nasa.gov/rss/api/?feed=weather&category=msl&feedtype=json), retrieved 2026-11-14, has newest record **2026-08-25 / sol 4995**:

- Daily low/high air temperature: **−71 / −5 °C**.
- Daily low/high ground temperature: **−84 / 4 °C**.
- Surface pressure: **777 Pa**.

The latest 30 records are bundled for provenance. `--` fields remain unavailable. REMS/CAB explicitly says this feed is for outreach, readings are affected by rover orientation, shadows, dust, and sampling schedules, and science users should obtain calibrated PDS data. That disclaimer is retained in `assets/data/weather_snapshots.json`.

### Radiation reference — Gale only

[NASA's 2013 Curiosity results](https://www.nasa.gov/news-release/nasa-rover-results-include-first-age-measurement-on-mars-and-help-for-human-exploration/) report **0.67 mSv per Earth day**, averaged over August 2012–June 2013. This historical period mean is labeled separately from weather and is not a present dose rate, personalized astronaut dose, shelter map, or Jezero value.

## Explicit coverage gaps

The requested feature list is represented in the catalog even when a dataset is not available. Null values with explanatory coverage notes are deliberately preserved. The bundled snapshot does **not** supply:

- Operational weather forecasts, live dust-storm tracks, optical depth, lifting/deposition rates, or validated wind/humidity measurements for the selected locations.
- A numeric stereo DEM, terrain slope/aspect, baseline-specific roughness, rock coverage, verified caves/lava entrances, or obstacle clearance.
- Local subsurface ice depth, water/hydrogen fraction, perchlorate concentrations, soil chemistry maps, irradiance, thermal inertia, calibrated albedo, or hourly thermal rates.
- Local atmospheric density profiles, water columns, gas-mixing-ratio observations, or scale heights.
- Local ionizing radiation at Jezero, magnetic pockets that provide protection, or current solar particle event alerts.
- Validated spacecraft relay schedules, blackout-zone geometry, entry/descent landing-ellipse polygons, terrain-aware shadows/glare, or Phobos/Deimos ephemerides.
- Real astronaut telemetry, life-support/oxygen certification, or automatic mission authorization.

These features require validated input products and are never populated with invented sample values. Ground-truth safety limits and travel speeds supplied by a user are planning assumptions, not NASA measurements.

## Bundled data files

- `assets/data/missions.json`: application-ready source-linked missions, observations, landmarks, and simplified historical traverses.
- `assets/data/weather_snapshots.json`: source weather records and retained REMS disclaimer.
- `assets/data/jezero_waypoint.geojson`, `gale_waypoint.geojson`: unmodified current waypoint source snapshots.
- `assets/data/source_manifest.json`: original source URLs and full-download SHA-256 checksums.
- `assets/maps/manifest.json`: map geometry, sources, credits, and checksums.

No NASA logo, endorsement, authentication key, or private mission data is included.

## Optional runtime refresh

`services::nasa::refresh_site` fetches the same public current-waypoint, traverse, and weather feeds on background workers. It uses a 15-second overall timeout and an 8 MiB response cap for each source. Sources refresh independently; a network, HTTP, size, or parse failure retains that source's previous data and reports its URL and error. A successful fetch updates the retrieval date, but keeps the source's mission sol and actual historical observation dates. Source records without an explicit UTC date remain undated in UTC.

Refreshed traverse geometry keeps the full published vertex sequence (deduplicating adjacent identical coordinates), so vertex counts can be larger than the simplified bundled catalog. Existing map images, bounds, gazetteer labels, and static historical landmarks stay unchanged. If a rover or traverse extends beyond bundled imagery, the real coordinates are retained and a coverage warning is reported; they are not clipped, relocated, or presented as covered terrain. Runtime refreshed values are public outreach/map products with the same scientific limitations described above.

## Decorative home panorama

`assets/maps/mars_panorama.jpg` is a resized copy of [NASA/JPL Mastcam-Z’s First 360-Degree Panorama (PIA24264)](https://www.jpl.nasa.gov/images/pia24264-mastcam-zs-first-360-degree-panorama/), published February 24, 2021. Credit: NASA/JPL-Caltech/MSSS/ASU. The scrolling blur is a decorative display effect, not georeferenced map information. Original dimensions, downloaded asset URL and checksum are recorded in `assets/maps/panorama_source.json`.

## Regional model outputs

See [CLIMATE_MODEL.md](CLIMATE_MODEL.md) for the credited MCD 6.2 grids, exact queries, raw outputs, conversions and limitations. Predictions are separate from measured NASA snapshots. The decorative background uses a terrain crop of NASA’s [Curiosity panorama PIA23623](https://science.nasa.gov/photojournal/curiositys-18-billion-pixel-panorama/), observed 2019-11-24 through 2019-12-01. It has a decorative warm red grade, periodic edge blending and offline blur; preprocessing and checksums are in `assets/maps/panorama_source.json`. Scientific map colors are unchanged.


Numerical MOLA terrain crops now support regional slope exclusions and route screening. See [TERRAIN_ROUTING.md](TERRAIN_ROUTING.md) for product labels, exact crop provenance, the 15° application policy and resolution limits.
