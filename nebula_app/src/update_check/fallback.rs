//! GitHub's public release pages are independent of the REST API quota.
//! The redirect proves a version; a SHA256SUMS asset can then restore verified
//! package metadata. Custom sources stay bound to the user-selected repository.

use std::time::Duration;

use ureq::ResponseExt as _;

use super::{LatestRelease, release_version_from_tag, source::ReleaseSource};
use crate::i18n::{LanguagePreference, Message};

pub(super) fn fetch(source: &ReleaseSource, status: u16) -> Result<LatestRelease, String> {
    log::info!(
        "update-check: API returned HTTP {status}; checking GitHub release page {}",
        source.probe_page()
    );
    let page = source.probe_page();
    let agent = crate::update_proxy::agent(&page, Duration::from_secs(10));
    let language =
        LanguagePreference::from(nebula_settings::RuntimeSettings::load().language).resolved();
    let uri = redirected_uri(&agent, &page).map_err(|error| {
        language.format(
            Message::UpdateCheckFallbackFailed,
            &[("status", &status.to_string()), ("error", &error.to_string())],
        )
    })?;
    let mut release = release_from_uri(source, &uri).ok_or_else(|| {
        if source.is_custom() {
            "GitHub Release 地址没有返回可识别的 Pebrel 版本".to_owned()
        } else {
            language.text(Message::UpdateCheckUnrecognizedRelease).to_owned()
        }
    })?;

    let url = format!(
        "https://github.com/{}/releases/download/{}/SHA256SUMS",
        source.repository(),
        release.tag
    );
    let agent = crate::update_proxy::agent(&url, Duration::from_secs(10));
    if let Ok(mut response) = agent.get(&url).header("User-Agent", "pebrel").call()
        && let Ok(text) = response.body_mut().with_config().limit(64 * 1024).read_to_string()
    {
        release.asset = super::assets::from_checksums(
            &release.version,
            &text,
            source.repository(),
            &release.tag,
        );
    }
    Ok(release)
}

pub(super) fn fetch_latest(status: u16) -> Result<LatestRelease, String> {
    fetch(&ReleaseSource::official(), status)
}

fn redirected_uri(agent: &ureq::Agent, url: &str) -> Result<String, ureq::Error> {
    let response = agent
        .get(url)
        .header("User-Agent", "pebrel")
        .header("Accept", "text/html")
        .config()
        .max_redirects(5)
        .build()
        .call()?;
    Ok(response.get_uri().to_string())
}

fn release_from_uri(source: &ReleaseSource, uri: &str) -> Option<LatestRelease> {
    let prefix = format!("https://github.com/{}/releases/tag/", source.repository());
    let tag = uri.strip_prefix(&prefix)?;
    if !super::source::valid_tag(tag) || source.tag().is_some_and(|expected| expected != tag) {
        return None;
    }
    let version = release_version_from_tag(tag)?;
    if !source.is_custom() && !stable_version(&version) {
        return None;
    }
    Some(LatestRelease { version, tag: tag.to_owned(), asset: None })
}

fn stable_version(version: &str) -> bool {
    let components: Vec<_> = version.split('.').collect();
    components.len() == 3
        && components.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && part.parse::<u64>().is_ok()
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_proxy::test_support::{Server, response};

    #[test]
    fn rate_limited_api_falls_back_and_discovers_an_upgrade_without_install_authority() {
        let source = ReleaseSource::official();
        for status in ["403 Forbidden", "429 Too Many Requests"] {
            let server = Server::start(vec![response(status, "", "API rate limit exceeded")]);
            let result = super::super::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |code| {
                    assert!(matches!(code, 403 | 429));
                    Ok(release_from_uri(
                        &source,
                        "https://github.com/Kuddev/pebrel/releases/tag/v1.9.0",
                    )
                    .unwrap())
                },
            )
            .unwrap();
            assert!(super::super::is_newer(&result.version, "1.8.2"));
            assert!(!super::super::is_newer(&result.version, "1.9.0"));
            assert!(result.asset.is_none(), "a version redirect grants no install authority");
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[test]
    fn successful_api_and_other_errors_never_call_the_fallback() {
        for (status, body, succeeds) in [
            ("200 OK", r#"{"tag_name":"v1.9.0","assets":[]}"#, true),
            ("200 OK", "not JSON", false),
            ("404 Not Found", "missing release", false),
            ("500 Internal Server Error", "server error", false),
        ] {
            let server = Server::start(vec![response(status, "", body)]);
            let result = super::super::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |_| panic!("fallback is only for access/rate limiting"),
            );
            assert_eq!(result.is_ok(), succeeds);
            server.finish();
        }
    }

    #[test]
    fn failed_fallback_stays_an_error_instead_of_claiming_up_to_date() {
        let server = Server::start(vec![response("403 Forbidden", "", "rate limited")]);
        let result = super::super::fetch_release_with_fallback(
            &server.agent(&[]),
            "http://api.update.invalid/latest",
            |_| Err("release page unavailable".into()),
        );
        assert_eq!(result.unwrap_err(), "release page unavailable");
        server.finish();
    }

    #[test]
    fn redirect_transport_uses_the_final_location_without_parsing_html() {
        let destination = "http://github.com/Kuddev/pebrel/releases/tag/v1.9.0";
        let server = Server::start(vec![
            response("302 Found", &format!("Location: {destination}\r\n"), ""),
            response("200 OK", "Content-Type: text/html\r\n", "<html>no metadata</html>"),
        ]);
        let uri =
            redirected_uri(&server.agent(&[]), "http://github.com/Kuddev/pebrel/releases/latest")
                .unwrap();
        assert_eq!(uri, destination);
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].1.contains("/releases/latest"));
        assert!(requests[1].1.contains("/releases/tag/v1.9.0"));
    }

    #[test]
    fn official_fallback_accepts_only_stable_tags_from_the_official_repository() {
        let source = ReleaseSource::official();
        for tag in ["v1.9.0", "V1.9.0", "1.9.0"] {
            assert_eq!(
                release_from_uri(
                    &source,
                    &format!("https://github.com/Kuddev/pebrel/releases/tag/{tag}")
                )
                .unwrap()
                .version,
                "1.9.0"
            );
        }
        for uri in [
            "https://github.com/Kuddev/pebrel/releases/latest",
            "https://github.com/login",
            "https://github.com/Other/pebrel/releases/tag/v99.0.0",
            "http://github.com/Kuddev/pebrel/releases/tag/v1.9.0",
            "https://github.com.evil.invalid/Kuddev/pebrel/releases/tag/v1.9.0",
            "https://github.com/Kuddev/pebrel/releases/tag/v1.9.0?foo=bar",
            "https://github.com/Kuddev/pebrel/releases/tag/v1.9.0/extra",
            "https://github.com/Kuddev/pebrel/releases/tag/v1.9.0-rc1",
            "https://github.com/Kuddev/pebrel/releases/tag/v01.9.0",
        ] {
            assert!(release_from_uri(&source, uri).is_none(), "{uri}");
        }
    }

    #[test]
    fn custom_exact_tag_fallback_accepts_a_semver_style_prerelease() {
        let source = ReleaseSource::from_setting(
            "https://github.com/acme/pebrel/releases/tag/v2.0.0-preview.7",
        )
        .unwrap();
        let release = release_from_uri(
            &source,
            "https://github.com/acme/pebrel/releases/tag/v2.0.0-preview.7",
        )
        .unwrap();
        assert_eq!(release.version, "2.0.0-preview.7");
        assert_eq!(release.tag, "v2.0.0-preview.7");
        assert!(
            release_from_uri(
                &source,
                "https://github.com/acme/pebrel/releases/tag/v2.0.0-preview.8"
            )
            .is_none()
        );
    }

    #[test]
    #[ignore = "contacts public GitHub; run explicitly for native network acceptance"]
    fn live_update_check_survives_api_rate_limiting() {
        let release = super::super::check_now().expect("live public update check");
        println!(
            "current={} latest={} update_available={}",
            release.current, release.latest, release.update_available
        );
        let fallback = fetch_latest(403).expect("live official release fallback");
        assert_eq!(release.latest, fallback.version);
    }
}
