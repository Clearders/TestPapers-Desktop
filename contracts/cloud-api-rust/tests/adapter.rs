use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use testpapers_cloud_api::adapter::CloudApi;
use testpapers_cloud_api::apis::drafts_api::DownloadDraftParams;
use testpapers_cloud_api::apis::sync_api::{self, PullSyncChangesParams};

const DOCX_BYTES: &[u8] = &[0x50, 0x4b, 0x03, 0x04, 0x00, 0xff];

fn spawn_download_server() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("read test server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buffer = [0_u8; 4096];
        let size = stream.read(&mut buffer).expect("read request");
        let request = String::from_utf8_lossy(&buffer[..size]).into_owned();

        let headers = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Length: 6\r\n",
            "Content-Type: application/vnd.openxmlformats-officedocument.wordprocessingml.document\r\n",
            "Content-Disposition: attachment; filename=cloud-draft.docx\r\n",
            "X-Cloud-Draft-Export: true\r\n",
            "X-Layout-Density: dense\r\n",
            "Connection: close\r\n",
            "\r\n"
        );
        stream
            .write_all(headers.as_bytes())
            .expect("write response headers");
        stream.write_all(DOCX_BYTES).expect("write response body");
        request
    });
    (format!("http://{address}/"), server)
}

fn spawn_json_server(body: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("read test server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buffer = [0_u8; 8192];
        let size = stream.read(&mut buffer).expect("read request");
        let request = String::from_utf8_lossy(&buffer[..size]).into_owned();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        request
    });
    (format!("http://{address}/"), server)
}

#[tokio::test]
async fn bearer_auth_and_binary_response_are_preserved() {
    let (base_path, server) = spawn_download_server();
    let client = CloudApi::new(base_path, "desktop-secret");

    let download = client
        .download_draft(DownloadDraftParams {
            draft_public_id: "draft-public-id".to_owned(),
            format: None,
        })
        .await
        .expect("download succeeds");
    let request = server.join().expect("server finishes");
    let lowercase_request = request.to_ascii_lowercase();

    assert!(request.starts_with("GET /api/v1/drafts/draft-public-id/download HTTP/1.1"));
    assert!(lowercase_request.contains("authorization: bearer desktop-secret\r\n"));
    assert_eq!(download.status, reqwest::StatusCode::OK);
    assert_eq!(download.bytes, DOCX_BYTES);
    assert_eq!(
        download.headers.get("content-disposition").unwrap(),
        "attachment; filename=cloud-draft.docx"
    );
    assert_eq!(
        download.headers.get("x-cloud-draft-export").unwrap(),
        "true"
    );
    assert_eq!(download.headers.get("x-layout-density").unwrap(), "dense");
}

#[tokio::test]
async fn sync_pull_uses_api_v1_query_shape_and_bearer_authentication() {
    let body = r#"{"success":true,"data":{"protocolVersion":1,"changes":[],"nextCursor":"opaque-next","hasMore":false},"meta":{"requestId":"request-id"}}"#;
    let (base_path, server) = spawn_json_server(body);
    let client = CloudApi::new(base_path, "native-access-token");

    let response = sync_api::pull_sync_changes(
        client.configuration(),
        PullSyncChangesParams {
            cursor: Some("opaque-current".into()),
            page_size: Some(75),
        },
    )
    .await
    .expect("pull succeeds");
    let request = server.join().expect("server finishes");
    let lowercase_request = request.to_ascii_lowercase();

    assert!(request.starts_with("GET /api/v1/sync/pull?cursor=opaque-current&pageSize=75 HTTP/1.1"));
    assert!(lowercase_request.contains("authorization: bearer native-access-token\r\n"));
    assert_eq!(response.data.protocol_version, 1);
    assert_eq!(response.data.next_cursor, "opaque-next");
}
