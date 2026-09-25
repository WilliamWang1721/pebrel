use super::{RELEASES_PAGE, UpdateAsset};

const GITHUB_PREFIX: &str = "https://github.com/";
const OFFICIAL_REPOSITORY: &str = "Kuddev/pebrel";
const LEGACY_REPOSITORY: &str = "Kuddev/nebula";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReleaseSource {
    repository: String,
    tag: Option<String>,
    custom: bool,
}

impl ReleaseSource {
    pub(super) fn official() -> Self {
        Self {
            repository: OFFICIAL_REPOSITORY.to_owned(),
            tag: None,
            custom: false,
        }
    }

    pub(super) fn from_setting(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(Self::official());
        }
        let value = value.trim_end_matches('/');
        let rest = value
            .strip_prefix(GITHUB_PREFIX)
            .ok_or_else(|| "自定义更新地址必须是 https://github.com 上的 Release 地址".to_owned())?;
        if rest.contains('?') || rest.contains('#') {
            return Err("自定义更新地址不能包含查询参数或片段".to_owned());
        }
        let parts: Vec<_> = rest.split('/').collect();
        let (owner, repository, tag) = match parts.as_slice() {
            [owner, repository] | [owner, repository, "releases"] => {
                (*owner, *repository, None)
            },
            [owner, repository, "releases", "latest"] => (*owner, *repository, None),
            [owner, repository, "releases", "tag", tag] => {
                (*owner, *repository, Some((*tag).to_owned()))
            },
            _ => return Err(
                "请填写 GitHub 仓库的 Releases、releases/latest 或 releases/tag/<tag> 地址"
                    .to_owned(),
            ),
        };
        if !valid_repository_component(owner) || !valid_repository_component(repository) {
            return Err("GitHub 仓库地址无效".to_owned());
        }
        if tag.as_deref().is_some_and(|tag| !valid_tag(tag)) {
            return Err("GitHub Release tag 无效".to_owned());
        }
        Ok(Self {
            repository: format!("{owner}/{repository}"),
            tag,
            custom: true,
        })
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
            Some(tag) => {
                format!("https://api.github.com/repos/{}/releases/tags/{tag}", self.repository)
            },
            None => format!("https://api.github.com/repos/{}/releases/latest", self.repository),
        }
    }

    pub(super) fn probe_page(&self) -> String {
        match self.tag() {
            Some(tag) => {
                format!("https://github.com/{}/releases/tag/{tag}", self.repository)
            },
            None => format!("https://github.com/{}/releases/latest", self.repository),
        }
    }

    pub(super) fn release_page(&self) -> String {
        match self.tag() {
            Some(tag) => {
                format!("https://github.com/{}/releases/tag/{tag}", self.repository)
            },
            None if self.custom => format!("https://github.com/{}/releases", self.repository),
            None => RELEASES_PAGE.to_owned(),
        }
    }

    pub(super) fn normalized_setting(&self) -> String {
        if !self.custom {
            String::new()
        } else {
            self.release_page()
        }
    }

    pub(super) fn accepts_download(&self, asset: &UpdateAsset) -> bool {
        if !self.custom {
            return [OFFICIAL_REPOSITORY, LEGACY_REPOSITORY].iter().any(|repository| {
                asset.download_url
                    == format!(
                        "https://github.com/{repository}/releases/download/v{}/{}",
                        asset.version, asset.name
                    )
            });
        }

        let prefix = format!("https://github.com/{}/releases/download/", self.repository);
        let Some(rest) = asset.download_url.strip_prefix(&prefix) else {
            return false;
        };
        let mut parts = rest.split('/');
        let (Some(tag), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        valid_tag(tag) && name == asset.name
    }
}

fn valid_repository_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(super) fn valid_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+')
        })
}

pub(super) fn configured() -> Result<ReleaseSource, String> {
    ReleaseSource::from_setting(&nebula_settings::RuntimeSettings::load().update_release_url)
}

pub(crate) fn normalize_setting(value: &str) -> Result<String, String> {
    Ok(ReleaseSource::from_setting(value)?.normalized_setting())
}

pub(crate) fn configured_release_page() -> Result<String, String> {
    Ok(configured()?.release_page())
}

pub(crate) fn validate_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    if configured()?.accepts_download(asset) {
        Ok(())
    } else {
        Err("release 安装包 URL 不属于当前更新源".to_owned())
    }
}

#[cfg(test)]
pub(crate) fn validate_official_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    if ReleaseSource::official().accepts_download(asset) {
        Ok(())
    } else {
        Err("release 安装包 URL 不属于 Pebrel 官方仓库".to_owned())
    }
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
    fn custom_release_pages_normalize_without_losing_an_explicit_tag() {
        for value in [
            "https://github.com/acme/pebrel",
            "https://github.com/acme/pebrel/releases",
            "https://github.com/acme/pebrel/releases/latest",
        ] {
            assert_eq!(
                normalize_setting(value).unwrap(),
                "https://github.com/acme/pebrel/releases"
            );
        }
        assert_eq!(
            normalize_setting("https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1").unwrap(),
            "https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1"
        );
    }

    #[test]
    fn custom_source_only_authorizes_release_assets_from_that_repository() {
        let source =
            ReleaseSource::from_setting("https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1")
                .unwrap();
        assert!(source.accepts_download(&asset(
            "https://github.com/acme/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe"
        )));
        for url in [
            "https://github.com/other/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
            "http://github.com/acme/pebrel/releases/download/v2.0.0-beta.1/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
            "https://github.com/acme/pebrel/releases/download/v2.0.0-beta.1/extra/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe",
        ] {
            assert!(!source.accepts_download(&asset(url)), "{url}");
        }
    }

    #[test]
    fn non_github_and_malformed_release_sources_are_rejected() {
        for value in [
            "https://example.com/acme/pebrel/releases",
            "http://github.com/acme/pebrel/releases",
            "https://github.com/acme/pebrel/issues",
            "https://github.com/acme/pebrel/releases/tag/a/b",
            "https://github.com/acme/pebrel/releases?tab=readme",
        ] {
            assert!(normalize_setting(value).is_err(), "{value}");
        }
    }
}
