# Research brief coverage and evidence limits

The user-provided **Space Apps 2026.docx** is a feature brief, not a scientific dataset. Its generic Mars values and example ranges are not treated as local measurements. `DATA_SOURCES.md` records the actual downloaded data and `CLIMATE_MODEL.md` documents regional predictions separately from measured observations.

| Requested capability | Implementation and evidence boundary |
| --- | --- |
| macOS, Windows, Linux | Shared Rust/GPUI application and a build matrix on matching GitHub-hosted operating systems. A Linux build does not verify macOS/Windows binaries. |
| Two crater choices | Jezero and Gale are separate mission contexts with their own real map imagery, recorded rover paths, landmarks, and observations. |
| Maps-style browsing | Georeferenced orbital-image map with pan, zoom, feature selection, and source attribution. The selected map point is not a weather station. |
| Mars and Earth clocks | UTC Earth time and calculated local mean Mars time; header toggles the view. Mars Sol Date is a continuous planetary count, not a Perseverance or Curiosity mission sol. |
| Mars year, seasons, AMT, day/night | NASA GISS Mars24 orbital/time formulas provide Ls, Airy Mean Time, solar altitude and daylight estimate. The implementation includes historical TAI−UTC offsets through 2017; future leap-second changes require updating the table. |
| Shadows/glare | A service can calculate a geometric shadow length from object height and solar altitude on flat ground. Terrain shadows, dust extinction, glare, and spacecraft illumination are not inferred. |
| Rover locations and legacy hardware | Published dated/sol-tagged rover-track samples are shown as **last recorded**, not live positions. Only hardware with sourced coordinates belongs in the landmark catalog. |
| Multiple routes and ETA | MOLA-screened direct or A* terrain-avoidance route, plus a recorded-rover alternative only when its connectors and all segments pass the same slope screen. Distance uses NASA's 3389.5 km mean Mars radius; ETA uses an explicit user speed assumption, not an unmeasured rover or astronaut speed. |
| Obstacle avoidance | Numerical MOLA slopes ≥15° are application planning exclusions; new routes avoid them and unassessed cells. At ~463 m spacing this does not resolve small hazards or establish safety. See TERRAIN_ROUTING.md. |
| Mission log | Successful route calculations and explicitly selected alternatives are automatically persisted with start/end labels and coordinates, geometry, distance, ETA, assumed speed, mode, and UTC planning time. These are plans, not physical visits. Saved-route previews open inside Mission log and do not replace the explorer’s camera, selection, endpoints or route. Legacy location entries remain in storage. |
| Astronaut location | Removed from the current app at user request. |
| Hands-free commands | Microphone controls have been removed at user request. The archived bridge utility is not connected to the desktop UI. |
| Earth-to-Mars information upload | Packet import and watched inbox removed at user request. |
| Metric/scientific units | Celsius versus kelvin, kilometres versus metres, km/h versus m/s, hPa versus Pa. Both use SI values; changing display units never changes underlying measurements. |
| Optical, thermal, atmospheric context | Real orbital imagery and dated, source-tagged environmental observations. Source/site coverage remains visible; a site observation does not become a sample at every selected pixel. |
| Air/surface temperatures, wind and pressure | Historical observations remain dated. Missing supported fields use MCD 6.2 regional seasonal/diurnal climatology, individually labelled Predicted. These are not live forecasts or pixel measurements. |
| Atmospheric density | Regional MCD 6.2 density is labelled Predicted; historical observations are not mixed to derive density. |
| Future conditions and forecasts | MCD regional climatology provides seasonal/diurnal predictions, not operational forecasts. Historical samples remain separate. |
| Dust storm tracking, optical depth and deposition | MCD regional optical depth and deposition are labelled Predicted. No operational storm-tracking feed is claimed. |
| Elevation, slopes and roughness | Separate numerical PDS MOLA grids supply regional slope estimates. The color relief image is decorative context, not the elevation input. Small-scale roughness remains unassessed. |
| Caves, lava and subsurface ice | No sufficiently resolved, co-registered inventory is bundled; unavailable rather than speculative hazard/ice overlays. |
| Chemistry and perchlorate alerts | A published regional/instrument result can provide context, but no pixel-scale toxic-dust sensor feed is claimed. |
| Radiation and magnetic pockets | Site-scale archival context may be presented with its source/date. No live dose sensor, shielding model, magnetic safe zone or astronaut dose prediction is inferred. |
| Surface irradiance, thermal inertia and albedo | Regional MCD surface solar irradiance, thermal inertia and albedo are labelled Predicted. Solar altitude remains a separate calculated quantity. |
| Gas mixing ratios, humidity and water column | MCD supplies regional CO₂ mixing ratio and water column. Relative humidity over ice is calculated from MCD water mixing ratio, pressure and temperature using Murphy–Koop ice saturation. Each is labelled Predicted; see CLIMATE_MODEL.md. |
| Entry/descent/landing ellipses | Landing points are sourced. An ellipse requires published georeferenced axes/orientation or geometry; no invented outline is represented as an operational landing zone. |
| Communications relay and blackout zones | Requires current orbiter ephemerides, a horizon/terrain model, antennas and link budget. No false communications-coverage guarantee. |
| Phobos/Deimos tracks | Requires an appropriate SPICE ephemeris and frame/time conversion. No decorative orbital tracks are passed off as calculated ground tracks. |

## Calculation references

- [NASA GISS Mars24 algorithm and verification examples](https://www.giss.nasa.gov/tools/mars24/help/algorithm.html): UTC-to-TT, MSD, AMT/LMST/LTST, Ls, solar declination, and altitude. The published 2000-01-06 worked example is covered by automated tests.
- [NASA GISS technical notes](https://www.giss.nasa.gov/tools/mars24/help/notes.html): Mars solar time conventions and accuracy limits. Local topography and atmospheric refraction are not included in the flat-surface Sun estimate.
- [NASA Mars fact sheet](https://nssdc.gsfc.nasa.gov/planetary/factsheet/marsfact.html): mean Mars radius for spherical distance, and equatorial/polar radii for planetocentric-to-planetographic conversion.

The map supports mission research and planning. Missing telemetry and incomplete hazard products must stay visible as gaps; the interface is not certified astronaut navigation or a substitute for mission flight rules.
