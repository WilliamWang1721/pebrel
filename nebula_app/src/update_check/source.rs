use super::{RELEASES_PAGE, UpdateAsset};

const GITHUB: &str = "https://github.com/";
const OFFICIAL: &str = "Kuddev/pebrel";
const LEGACY: &str = "Kuddev/nebula";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReleaseSource {
    repository: String,
    tag: Option<String>,
    custom: bool,
}

impl ReleaseSource {
    pub(super) fn official() -> Self {
        Self { repository: OFFICIAL.into(), tag: None, custom: false }
    }

    pub(super) fn from_setting(value: &str) -> Result<Self, String> {
        let value = value.trim().trim_end_matches('/');
        if value.is_empty() {
            return Ok(Self::official());
        }
        let rest = value
            .strip_prefix(GITHUB)
            .ok_or_else(|| "自定义更新地址必须使用 https://github.com".to_owned())?;
        if rest.contains(['?', '#']) {
            return Err("自定义更新地址不能包含查询参数或片段".into());
        }
        let parts: Vec<_> = rest.split('/').collect();
        let (owner, repo, tag) = match parts.as_slice() {
            [owner, repo] | [owner, repo, "releases"] | [owner, repo, "releases", "latest"] => {
                (*owner, *repo, None)
            },
            [owner, repo, "releases", "tag", tag] => (*owner, *repo, Some((*tag).to_owned())),
            _ => {
                return Err(
                    "请填写 GitHub 仓库的 Releases、releases/latest 或 releases/tag/<tag> 地址"
                        .into(),
                );
            },
        };
        if !valid_component(owner) || !valid_component(repo) {
            return Err("GitHub 仓库地址无效".into());
        }
        if tag.as_deref().is_some_and(|tag| !valid_tag(tag)) {
            return Err("GitHub Release tag 无效".into());
        }
        Ok(Self { repository: format!("{owner}/{repo}"), tag, custom: true })
    }

    pub(super) fn repository(&self) -> &str {
        &self.repository
    }

    pub(super) fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub(super) fn is_custom(&self) -> bool {
        self.custom
    }

    pub(super) fn api_url(&self) -> String {
        match self.tag() {
            Some(tag) => format!("https://api.github.com/repos/{}/releases/tags/{tag}", self.repository),
            None => format!("https://api.github.com/repos/{}/releases/latest", self.repository),
        }
    }

    pub(super) fn probe_page(&self) -> String {
        match self.tag() {
            Some(tag) => format!("{GITHUB}{}/releases/tag/{tag}", self.repository),
            None => format!("{GITHUB}{}/releases/latest", self.repository),
        }
    }

    pub(super) fn release_page(&self) -> String {
        match self.tag() {
            Some(tag) => format!("{GITHUB}{}/releases/tag/{tag}", self.repository),
            None if self.custom => format!("{GITHUB}{}/releases", self.repository),
            None => RELEASES_PAGE.into(),
        }
    }

    fn accepts_download(&self, asset: &UpdateAsset) -> bool {
        if !self.custom {
            return [OFFICIAL, LEGACY].iter().any(|repository| {
                asset.download_url
                    == format!("{GITHUB}{repository}/releases/download/v{}/{}", asset.version, asset.name)
            });
        }
        let prefix = format!("{GITHUB}{}/releases/download/", self.repository);
        let Some(rest) = asset.download_url.strip_prefix(&prefix) else { return false };
        let mut parts = rest.split('/');
        let (Some(tag), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        valid_tag(tag)
            && name == asset.name
            && self.tag().is_none_or(|configured| configured == tag)
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(super) fn valid_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
}

pub(super) fn configured() -> Result<ReleaseSource, String> {
    ReleaseSource::from_setting(&nebula_settings::RuntimeSettings::load().update_release_url)
}

pub(crate) fn normalize_setting(value: &str) -> Result<String, String> {
    let source = ReleaseSource::from_setting(value)?;
    Ok(if source.is_custom() { source.release_page() } else { String::new() })
}

pub(crate) fn configured_release_page() -> Result<String, String> {
    Ok(configured()?.release_page())
}

pub(crate) fn validate_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    configured()?
        .accepts_download(asset)
        .then_some(())
        .ok_or_else(|| "release 安装包 URL 不属于当前更新源".to_owned())
}

#[cfg(test)]
pub(crate) fn validate_official_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    ReleaseSource::official()
        .accepts_download(asset)
        .then_some(())
        .ok_or_else(|| "release 安装包 URL 不属于 Pebrel 官方仓库".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(url: &str) -> UpdateAsset {
        UpdateAsset {
            version: "2.0.0-beta.1".into(),
            name: "Pebrel-v2.0.0-beta.1-windows-x64-setup.exe".into(),
            download_url: url.into(),
            size: Some(42),
            sha256: Some("a".repeat(64)),
        }
    }

    #[test]
    fn release_urls_normalize_and_reject_non_github_sources() {
        for url in [
            "https://github.com/acme/pebrel",
            "https://github.com/acme/pebrel/releases",
            "https://github.com/acme/pebrel/releases/latest",
        ] {
            assert_eq!(normalize_setting(url).unwrap(), "https://github.com/acme/pebrel/releases");
        }
        assert_eq!(
            normalize_setting("https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1").unwrap(),
            "https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1"
        );
        for url in [
            "https://example.com/acme/pebrel/releases",
            "http://github.com/acme/pebrel/releases",
            "https://github.com/acme/pebrel/issues",
        ] {
            assert!(normalize_setting(url).is_err(), "{url}");
        }
    }

    #[test]
    fn custom_source_authorizes_only_its_repository_and_exact_tag_when_selected() {
        let source =
            ReleaseSource::from_setting("https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1")
                .unwrap();
        assert!(source.accepts_download(&asset(
            "https://github.com/acme/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe"
        )));
        for url in [
            "https://github.com/acme/pebrel/releases/download/v2.0.0-beta.2/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
            "https://github.com/other/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
            "http://github.com/acme/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
        ] {
            assert!(!source.accepts_download(&asset(url)), "{url}");
        }
    }
}
