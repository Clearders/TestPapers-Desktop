//! Handwritten native-client boundary around the generated reqwest APIs.

use crate::apis::{self, configuration::Configuration, drafts_api, papers_api};
use reqwest::{header::HeaderMap, StatusCode};

/// A native Cloud client configured to send a bearer credential on generated requests.
#[derive(Clone)]
pub struct CloudApi {
    configuration: Configuration,
}

impl CloudApi {
    /// Build a client for `base_path` and inject `access_token` as Bearer authentication.
    pub fn new(base_path: impl Into<String>, access_token: impl Into<String>) -> Self {
        let mut configuration = Configuration::new();
        configuration.base_path = base_path.into().trim_end_matches('/').to_owned();
        configuration.bearer_access_token = Some(access_token.into());
        Self { configuration }
    }

    /// Access the generated configuration for JSON endpoints not wrapped here.
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Download a collaborative draft without decoding or rewriting its response body.
    pub async fn download_draft(
        &self,
        params: drafts_api::DownloadDraftParams,
    ) -> Result<BinaryDownload, apis::Error<drafts_api::DownloadDraftError>> {
        preserve_download(drafts_api::download_draft(&self.configuration, params).await).await
    }

    /// Render and download an unsaved paper draft without decoding its response body.
    pub async fn download_draft_paper(
        &self,
        params: papers_api::DownloadDraftPaperParams,
    ) -> Result<BinaryDownload, apis::Error<papers_api::DownloadDraftPaperError>> {
        preserve_download(papers_api::download_draft_paper(&self.configuration, params).await).await
    }

    /// Download a saved paper without decoding or rewriting its response body.
    pub async fn download_paper(
        &self,
        params: papers_api::DownloadPaperParams,
    ) -> Result<BinaryDownload, apis::Error<papers_api::DownloadPaperError>> {
        preserve_download(papers_api::download_paper(&self.configuration, params).await).await
    }
}

/// Exact bytes and response metadata returned by a successful binary endpoint.
#[derive(Clone, Debug)]
pub struct BinaryDownload {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub bytes: Vec<u8>,
}

async fn preserve_download<E>(
    response: Result<reqwest::Response, apis::Error<E>>,
) -> Result<BinaryDownload, apis::Error<E>> {
    let response = response?;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.bytes().await?.to_vec();
    Ok(BinaryDownload {
        status,
        headers,
        bytes,
    })
}
