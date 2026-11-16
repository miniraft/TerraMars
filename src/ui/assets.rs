use gpui::RenderImage;
use std::sync::Arc;

/// Decoded once, uploaded once by GPUI's image atlas, then reused for every pan.
pub struct Images {
    pub imagery: [Arc<RenderImage>; 2],
    /// Card-sized crops. The Gale preview is a single zoomed frame so the home
    /// card doesn't show the mosaic's stitched seams.
    pub cards: [Arc<RenderImage>; 2],
    pub terrain: [Arc<RenderImage>; 2],
    pub panorama: Arc<RenderImage>,
    /// Public-domain NASA Viking mosaic used by the Home "Coming Soon" card.
    pub coming_soon: Arc<RenderImage>,
    pub hazards: [Arc<terramars::domain::terrain::TerrainGrid>; 2],
}

fn decode(bytes: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    let mut rgba = image::load_from_memory(bytes)?.into_rgba8();
    for pixel in rgba.pixels_mut() {
        pixel.0.swap(0, 2); // GPUI uses BGRA.
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

impl Images {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            hazards: [
                terramars::domain::terrain::TerrainGrid::bundled("jezero")?,
                terramars::domain::terrain::TerrainGrid::bundled("gale")?,
            ],
            imagery: [
                decode(include_bytes!("../../assets/maps/jezero_ctx.jpg"))?,
                decode(include_bytes!("../../assets/maps/gale_ctx.jpg"))?,
            ],
            terrain: [
                decode(include_bytes!("../../assets/maps/jezero_mola.jpg"))?,
                decode(include_bytes!("../../assets/maps/gale_mola.jpg"))?,
            ],
            panorama: decode(include_bytes!("../../assets/maps/mars_panorama_loop.jpg"))?,
            cards: [
                decode(include_bytes!("../../assets/maps/jezero_ctx.jpg"))?,
                decode(include_bytes!("../../assets/maps/gale_card.jpg"))?,
            ],
            coming_soon: decode(include_bytes!("../../assets/maps/coming_soon_mars.jpg"))?,
        })
    }
}

pub struct EmbeddedAssets;
impl gpui::AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "icons/back.svg" => Some(include_bytes!("../../assets/icons/back.svg")),
            "icons/info.svg" => Some(include_bytes!("../../assets/icons/info.svg")),
            "icons/rover.svg" => Some(include_bytes!("../../assets/icons/rover.svg")),
            "icons/fit.svg" => Some(include_bytes!("../../assets/icons/fit.svg")),
            "icons/layers.svg" => Some(include_bytes!("../../assets/icons/layers.svg")),
            "icons/pin.svg" => Some(include_bytes!("../../assets/icons/pin.svg")),
            "icons/refresh.svg" => Some(include_bytes!("../../assets/icons/refresh.svg")),
            "icons/chevron.svg" => Some(include_bytes!("../../assets/icons/chevron.svg")),
            "icons/route.svg" => Some(include_bytes!("../../assets/icons/route.svg")),
            "icons/metric.svg" => Some(include_bytes!("../../assets/icons/metric.svg")),
            "icons/scientific.svg" => Some(include_bytes!("../../assets/icons/scientific.svg")),
            "icons/mic.svg" => Some(include_bytes!("../../assets/icons/mic.svg")),
            _ => None,
        };
        Ok(bytes.map(std::borrow::Cow::Borrowed))
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        Ok([
            "back",
            "info",
            "rover",
            "fit",
            "layers",
            "pin",
            "refresh",
            "chevron",
            "route",
            "metric",
            "scientific",
            "mic",
        ]
        .into_iter()
        .map(|name| format!("icons/{name}.svg"))
        .filter(|name| name.starts_with(path))
        .map(Into::into)
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_asset_registry_can_resolve_every_control_icon() {
        // GPUI CE builds its registry from AssetSource::list, unlike the old
        // path lookup. Test the real registry boundary to prevent blank icons.
        let registry = gpui::AssetRegistry::from(EmbeddedAssets);
        for name in [
            "back",
            "info",
            "rover",
            "fit",
            "layers",
            "pin",
            "refresh",
            "chevron",
            "route",
            "metric",
            "scientific",
        ] {
            let path = format!("icons/{name}.svg");
            let bytes = registry
                .load(&path)
                .unwrap_or_else(|| panic!("Missing {path}"));
            assert!(std::str::from_utf8(&bytes).unwrap().contains("<svg"));
        }
    }
}
