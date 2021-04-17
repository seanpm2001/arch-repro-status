//! Package data from the Arch Linux website.

/// JSON endpoint to use for searching packages.
pub const ARCHWEB_ENDPOINT: &str = "https://archlinux.org/packages/search/json";

/// Search result from archlinux.org
#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub version: i64,
    pub limit: i64,
    pub valid: bool,
    pub results: Vec<ArchwebPackage>,
    #[serde(rename = "num_pages")]
    pub num_pages: Option<i64>,
    pub page: Option<i64>,
}

/// Package data that archlinux.org provides.
#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchwebPackage {
    pub pkgname: String,
    pub pkgbase: String,
    pub repo: String,
    pub arch: String,
    pub pkgver: String,
    pub pkgrel: String,
    pub epoch: i64,
    pub pkgdesc: String,
    pub url: String,
    pub filename: String,
    #[serde(rename = "compressed_size")]
    pub compressed_size: i64,
    #[serde(rename = "installed_size")]
    pub installed_size: i64,
    #[serde(rename = "build_date")]
    pub build_date: String,
    #[serde(rename = "last_update")]
    pub last_update: String,
    #[serde(rename = "flag_date")]
    pub flag_date: ::serde_json::Value,
    pub maintainers: Vec<String>,
    pub packager: String,
    pub groups: Vec<::serde_json::Value>,
    pub licenses: Vec<String>,
    pub conflicts: Vec<::serde_json::Value>,
    pub provides: Vec<::serde_json::Value>,
    pub replaces: Vec<String>,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub makedepends: Vec<String>,
    pub checkdepends: Vec<::serde_json::Value>,
}
