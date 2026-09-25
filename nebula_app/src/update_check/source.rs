use super::{RELEASES_PAGE, UpdateAsset};

const GITHUB: &str = "https://github.com/";
const OFFICIAL: &str = "Kuddev/pebrel";
const LEGACY: &str = "Kuddev/nebula";

pub(super) struct ReleaseSource {
    repo: String,
    tag: Option<String>,
    default: bool,
}

impl ReleaseSource {
    fn official() -> Self {
        Self { repo: OFFICIAL.into(), tag: None, default: true }
    }

    fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim().trim_end_matches('/');
        if value.is_empty() {
            return Ok(Self::official());
        }
        let path = value
            .strip_prefix(GITHUB)
            .ok_or_else(|| "自定义更新地址必须使用 https://github.com".to_owned())?;
        if path.contains('?') || path.contains('#') {
            return Err("自定义更新地址不能包含查询参数或片段".into());
        }
        let parts: Vec<_> = path.split('/').collect();
        let (owner, repo, tag) = match parts.as_slice() {
            [owner, repo] | [owner, repo, "releases"] | [owner, repo, "releases", "latest"] => {
                (*owner, *repo, None)
            },
            [owner, repo, "releases", "tag", tag] => (*owner, *repo, Some((*tag).to_owned())),
            _ => return Err("请填写 GitHub Releases 或 releases/tag/<tag> 地址".into()),
        };
        if !component(owner) || !component(repo) || tag.as_deref().is_some_and(|tag| !tag_name(tag))
        {
            return Err("GitHub Release 地址无效".into());
        }
        Ok(Self { repo: format!("{owner}/{repo}"), tag, default: false })
    }

    pub(super) fn is_default(&self) -> bool {
        self.default
    }

    pub(super) fn api_url(&self) -> String {
        match &self.tag {
            Some(tag) => format!("https://api.github.com/repos/{}/releases/tags/{tag}", self.repo),
            None => format!("https://api.github.com/repos/{}/releases/latest", self.repo),
        }
    }

    fn page(&self) -> String {
        match &self.tag {
            Some(tag) => format!("{GITHUB}{}/releases/tag/{tag}", self.repo),
            None if self.default => RELEASES_PAGE.into(),
            None => format!("{GITHUB}{}/releases", self.repo),
        }
    }

    fn accepts(&self, asset: &UpdateAsset) -> bool {
        if self.default {
            return [OFFICIAL, LEGACY].iter().any(|repo| {
                asset.download_url
                    == format!("{GITHUB}{repo}/releases/download/v{}/{}", asset.version, asset.name)
            });
        }
        let prefix = format!("{GITHUB}{}/releases/download/", self.repo);
        let Some(path) = asset.download_url.strip_prefix(&prefix) else { return false };
        let mut parts = path.split('/');
        let (Some(tag), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        name == asset.name && tag_name(tag) && self.tag.as_deref().is_none_or(|value| value == tag)
    }
}

fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn tag_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
}

pub(super) fn configured() -> Result<ReleaseSource, String> {
    ReleaseSource::parse(&nebula_settings::RuntimeSettings::load().update_release_url)
}

pub(crate) fn normalize_setting(value: &str) -> Result<String, String> {
    let source = ReleaseSource::parse(value)?;
    Ok(if source.default { String::new() } else { source.page() })
}

pub(crate) fn release_page() -> String {
    configured().map(|source| source.page()).unwrap_or_else(|_| RELEASES_PAGE.into())
}

pub(crate) fn validate_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    configured()?
        .accepts(asset)
        .then_some(())
        .ok_or_else(|| "release 安装包 URL 不属于当前更新源".into())
}

#[cfg(test)]
pub(crate) fn validate_official_asset_url(asset: &UpdateAsset) -> Result<(), String> {
    ReleaseSource::official()
        .accepts(asset)
        .then_some(())
        .ok_or_else(|| "release 安装包 URL 不属于 Pebrel 官方仓库".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_release_urls_normalize_to_one_persisted_shape() {
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
        assert!(normalize_setting("https://example.com/acme/pebrel/releases").is_err());
    }

    #[test]
    fn explicit_tag_only_trusts_that_release() {
        let source =
            ReleaseSource::parse("https://github.com/acme/pebrel/releases/tag/v2.0.0-beta.1")
                .unwrap();
        let asset = |tag: &str| UpdateAsset {
            version: "2.0.0-beta.1".into(),
            name: "Pebrel-v2.0.0-beta.1-windows-x64-setup.exe".into(),
            download_url: format!(
                "https://github.com/acme/pebrel/releases/download/{tag}/Pebrel-v2.0.0-beta.1-windows-x64-setup.exe"
            ),
            size: Some(42),
            sha256: Some("a".repeat(64)),
        };
        assert!(source.accepts(&asset("v2.0.0-beta.1")));
        assert!(!source.accepts(&asset("v2.0.0-beta.2")));
    }
}
