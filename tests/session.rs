use ne_api::{Client, Error, MemoryTokenStore, SearchQuery, SignInCallback, Tokens};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client(server: &MockServer) -> Client {
    Client::builder("cid", "csecret")
        .api_base_url(server.uri())
        .build()
        .unwrap()
}

fn ok(body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

#[tokio::test]
async fn neauth_returns_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api_neauth"))
        .and(body_string_contains("uid=U1"))
        .and(body_string_contains("state=S1"))
        .and(body_string_contains("client_secret=csecret"))
        .respond_with(ok(json!({
            "result": "success", "access_token": "A1", "refresh_token": "R1",
            "uid": "U1", "company_name": "テスト", "pic_ne_id": "9"
        })))
        .mount(&server)
        .await;

    let c = client(&server).await;
    let cb = SignInCallback::from_query("uid=U1&state=S1").unwrap();
    let res = c.neauth_callback(&cb).await.unwrap();
    assert_eq!(res.tokens, Tokens::new("A1", "R1"));
    assert_eq!(res.company_name.as_deref(), Some("テスト"));
}

#[tokio::test]
async fn rotated_tokens_are_saved() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api_v1_master_shop/search"))
        .and(body_string_contains("access_token=OLD"))
        .and(body_string_contains("fields=shop_id%2Cshop_name"))
        .and(body_string_contains("shop_id-in=1%2C2"))
        .respond_with(ok(json!({
            "result": "success", "count": "1",
            "data": [{"shop_id": "1", "shop_name": "店舗"}],
            "access_token": "NEW", "refresh_token": "NEWR",
            "access_token_end_date": "2026-11-07 00:00:00",
            "refresh_token_end_date": "2026-11-09 00:00:00"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let store = MemoryTokenStore::new(Some(Tokens::new("OLD", "OLDR")));
    let session = client(&server).await.session(store.clone());
    let res = session
        .master_shop()
        .search(
            &SearchQuery::new()
                .fields(["shop_id", "shop_name"])
                .in_("shop_id", ["1", "2"]),
        )
        .await
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.data[0]["shop_name"], "店舗");

    let saved = store.get().unwrap();
    assert_eq!(saved.access_token, "NEW");
    assert_eq!(saved.refresh_token, "NEWR");
    assert_eq!(
        saved.access_token_end_date.as_deref(),
        Some("2026-11-07 00:00:00")
    );
}

#[tokio::test]
async fn tokens_saved_even_on_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api_v1_receiveorder_base/count"))
        .respond_with(ok(json!({
            "result": "error", "code": "003002", "message": "busy",
            "access_token": "NEW", "refresh_token": "NEWR"
        })))
        .mount(&server)
        .await;

    let store = MemoryTokenStore::new(Some(Tokens::new("OLD", "OLDR")));
    let session = client(&server).await.session(store.clone());
    let err = session
        .receiveorder_base()
        .count(&SearchQuery::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), Some("003002"));
    assert!(err.is_retryable());
    assert_eq!(store.get().unwrap().access_token, "NEW");
}

#[tokio::test]
async fn redirect_requires_reauth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ok(
            json!({"result": "redirect", "code": "002004", "message": "expired"}),
        ))
        .mount(&server)
        .await;

    let session = client(&server)
        .await
        .session_with_tokens(Tokens::new("A", "R"));
    let err = session.login_user().info().await.unwrap_err();
    assert!(matches!(err, Error::Redirect { .. }));
    assert!(err.requires_reauth());
}

#[tokio::test]
async fn missing_tokens_is_not_authenticated() {
    let server = MockServer::start().await;
    let session = client(&server).await.session(MemoryTokenStore::default());
    let err = session.system_tax().info().await.unwrap_err();
    assert!(matches!(err, Error::NotAuthenticated));
}

#[tokio::test]
async fn wait_flag_and_upload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api_v1_master_goods/upload"))
        .and(body_string_contains("wait_flag=1"))
        .and(body_string_contains("data_type=csv"))
        .respond_with(ok(json!({"result": "success", "que_id": "189"})))
        .mount(&server)
        .await;

    let c = Client::builder("cid", "cs")
        .api_base_url(server.uri())
        .wait_flag(true)
        .build()
        .unwrap();
    let session = c.session_with_tokens(Tokens::new("A", "R"));
    let res = session
        .master_goods()
        .upload([("data_type", "csv"), ("data", "syohin_code\nX\n")])
        .await
        .unwrap();
    assert_eq!(res.que_id, "189");
}

#[tokio::test]
async fn search_all_pages() {
    let server = MockServer::start().await;
    for (offset, n) in [(0, 2), (2, 2), (4, 1)] {
        let data: Vec<_> = (0..n)
            .map(|i| json!({"goods_id": format!("G{}", offset + i)}))
            .collect();
        Mock::given(method("POST"))
            .and(path("/api_v1_master_goods/search"))
            .and(body_string_contains(format!("offset={offset}&").as_str()))
            .respond_with(ok(
                json!({"result": "success", "count": n.to_string(), "data": data}),
            ))
            .expect(1)
            .mount(&server)
            .await;
    }

    let session = client(&server)
        .await
        .session_with_tokens(Tokens::new("A", "R"));
    let rows = session
        .master_goods()
        .search_all(&SearchQuery::new().fields(["goods_id"]).limit(2))
        .await
        .unwrap();
    let ids: Vec<_> = rows
        .iter()
        .map(|r| r["goods_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["G0", "G1", "G2", "G3", "G4"]);
}

#[tokio::test]
async fn execute_with_explicit_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api_v1_system_orderstatus/info"))
        .respond_with(ok(json!({
            "result": "error", "code": "004001", "message": "no permission",
            "access_token": "A2", "refresh_token": "R2"
        })))
        .mount(&server)
        .await;

    let c = client(&server).await;
    let ex = c
        .execute(
            &Tokens::new("A", "R"),
            "/api_v1_system_orderstatus/info",
            (),
        )
        .await;
    assert_eq!(ex.tokens.unwrap(), Tokens::new("A2", "R2"));
    assert_eq!(ex.result.unwrap_err().code(), Some("004001"));
}

#[test]
fn sign_in_url() {
    let c = Client::new("cid", "cs").unwrap();
    let url = c.sign_in_url(Some("https://localhost:3000/cb"), [("extra", "x")]);
    assert_eq!(
        url.as_str(),
        "https://base.next-engine.org/users/sign_in/?client_id=cid&redirect_uri=https%3A%2F%2Flocalhost%3A3000%2Fcb&extra=x"
    );
}
