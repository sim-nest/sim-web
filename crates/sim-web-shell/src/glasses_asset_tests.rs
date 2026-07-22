use crate::assets::asset_for;

#[test]
fn glasses_layout_assets_are_embedded() {
    let js = asset_text("/interpreter/glasses.js");
    for expected in [
        "class BrowserGlassesClient",
        "scene/stereo",
        "side-by-side",
        "maxPredictMs",
    ] {
        assert!(js.contains(expected), "glasses client missing {expected}");
    }
    let scene = asset_text("/interpreter/scene.js");
    for expected in [
        "scene/spatial",
        "scene/stereo",
        "scene/panel",
        "scene/glance",
    ] {
        assert!(scene.contains(expected), "Scene painter missing {expected}");
    }
    let css = asset_text("/styles/theme.css");
    for expected in [
        ".scene-stereo",
        ".scene-eye",
        ".scene-glance-card",
        "data-glasses-mode",
    ] {
        assert!(css.contains(expected), "glasses layout missing {expected}");
    }
}

fn asset_text(path: &str) -> String {
    let asset = asset_for(path).unwrap_or_else(|| panic!("{path} must be served"));
    std::str::from_utf8(asset.body).unwrap().to_owned()
}
