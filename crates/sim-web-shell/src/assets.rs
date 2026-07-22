//! Embedded WebUI assets.
//!
//! The shell and cookbook pages are embedded at compile time so the binary
//! serves a complete WebUI regardless of the working directory. The current
//! bundle paints Scene data and emits Intent values; a live runtime session
//! bridge can replace or extend these assets without changing this routing
//! contract.

/// A static asset resolved for an HTTP path: its bytes and MIME content type.
pub struct Asset {
    /// The response body.
    pub body: &'static [u8],
    /// The `Content-Type` header value.
    pub content_type: &'static str,
}

const INDEX_HTML: &str = include_str!("../web/index.html");
const COOKBOOK_HTML: &str = include_str!("../web/cookbook.html");
const ATELIER_HTML: &str = include_str!("../web/atelier.html");
const THEME_CSS: &str = include_str!("../web/styles/theme.css");
const COOKBOOK_CSS: &str = include_str!("../web/cookbook/cookbook.css");
const ATELIER_CSS: &str = include_str!("../web/atelier/atelier.css");
const BOOT_JS: &str = include_str!("../web/interpreter/boot.js");
const APP_JS: &str = include_str!("../web/interpreter/app.js");
const GLASSES_JS: &str = include_str!("../web/interpreter/glasses.js");
const SCENE_JS: &str = include_str!("../web/interpreter/scene.js");
const DIFF_JS: &str = include_str!("../web/interpreter/diff.js");
const INTENT_JS: &str = include_str!("../web/interpreter/intent.js");
const KEYMAP_JS: &str = include_str!("../web/interpreter/keymap.js");
const SESSION_JS: &str = include_str!("../web/interpreter/session.js");
const COOKBOOK_JS: &str = include_str!("../web/cookbook/cookbook.js");
const ATELIER_JS: &str = include_str!("../web/atelier/atelier.js");

const JS_CONTENT_TYPE: &str = "text/javascript; charset=utf-8";

/// Resolve a request path to an embedded asset, or `None` for a 404.
///
/// `/` and `/index.html` both resolve to the shell page. Only the small set of
/// embedded assets is routable; everything else is a miss and the server fails
/// closed with a 404 rather than touching the filesystem.
pub fn asset_for(path: &str) -> Option<Asset> {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    match path {
        "/" | "/index.html" => Some(Asset {
            body: INDEX_HTML.as_bytes(),
            content_type: "text/html; charset=utf-8",
        }),
        "/cookbook" | "/cookbook.html" => Some(Asset {
            body: COOKBOOK_HTML.as_bytes(),
            content_type: "text/html; charset=utf-8",
        }),
        "/atelier" | "/atelier.html" => Some(Asset {
            body: ATELIER_HTML.as_bytes(),
            content_type: "text/html; charset=utf-8",
        }),
        "/styles/theme.css" => Some(Asset {
            body: THEME_CSS.as_bytes(),
            content_type: "text/css; charset=utf-8",
        }),
        "/cookbook/cookbook.css" => Some(Asset {
            body: COOKBOOK_CSS.as_bytes(),
            content_type: "text/css; charset=utf-8",
        }),
        "/atelier/atelier.css" => Some(Asset {
            body: ATELIER_CSS.as_bytes(),
            content_type: "text/css; charset=utf-8",
        }),
        "/interpreter/boot.js" => Some(Asset {
            body: BOOT_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/app.js" => Some(Asset {
            body: APP_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/glasses.js" => Some(Asset {
            body: GLASSES_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/scene.js" => Some(Asset {
            body: SCENE_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/diff.js" => Some(Asset {
            body: DIFF_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/intent.js" => Some(Asset {
            body: INTENT_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/keymap.js" => Some(Asset {
            body: KEYMAP_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/interpreter/session.js" => Some(Asset {
            body: SESSION_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/cookbook/cookbook.js" => Some(Asset {
            body: COOKBOOK_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        "/atelier/atelier.js" => Some(Asset {
            body: ATELIER_JS.as_bytes(),
            content_type: JS_CONTENT_TYPE,
        }),
        _ => None,
    }
}
