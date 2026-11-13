pub struct AppPage {
    pub path: &'static str,
    pub sitemap_priority: &'static str,
    pub sitemap_changefreq: &'static str,
}

pub const APP_PAGES: &[AppPage] = &[AppPage {
    path: "/",
    sitemap_priority: "1.0",
    sitemap_changefreq: "weekly",
}];
