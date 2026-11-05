# ne_api (Rust)

[ネクストエンジン API](https://developer.next-engine.com/) の非同期 Rust クライアントです。

- ドキュメント記載の全エンドポイント（128 件）を型付きメソッドで提供
- ネクストエンジン特有のトークン更新（レスポンスごとに access_token / refresh_token が入れ替わる）を `TokenStore` トレイトで外部ストレージに委譲
- 検索条件ビルダー（`-eq` / `-in` / `-like` … 全演算子、カンマ区切りのクォート処理込み）
- `search_all` によるページング取得、`wait_flag`、エラーコードの再試行判定

Ruby 版: [infinity-octaver/ne_api](https://github.com/infinity-octaver/ne_api)

## インストール

```toml
[dependencies]
ne_api = { git = "https://github.com/infinity-octaver/ne_api_rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 構成

| 型 | 役割 |
| --- | --- |
| `Client` | アプリ単位。`client_id` / `client_secret`、HTTP 接続プール、ログイン URL 生成、`neauth`（uid・state → トークン）、トークンを明示的に渡す低レベル API `execute` |
| `TokenStore` | ユーザー 1 人分のトークンの読み書き。DB や KVS に合わせて実装する（`MemoryTokenStore` 同梱） |
| `Session` | `Client` + `TokenStore`。リクエストのたびにトークンを読み込み、変化があれば保存し（エラー時も）、同一セッション内のリクエストを直列化する |

## 認証

ネクストエンジンには「リフレッシュ用エンドポイント」がありません。access_token（1 日）の期限切れ後、refresh_token（3 日）付きで最初に送ったリクエストのレスポンスに **新しいトークン** が入って返り、古いトークンはその時点で使えなくなります。エラー時にも新トークンが返ることがあるため、公式ガイドは「毎回永続化したトークンを読み、レスポンスごとに保存する」「同一ユーザーで並列リクエストしない」ことを推奨しています。`Session` はこれをそのまま実装しています。

### 1. ログイン（uid・state の取得）

```rust
let client = ne_api::Client::new(client_id, client_secret)?;

// ブラウザをここへリダイレクト
let url = client.sign_in_url(Some("https://example.com/ne_auth/callback"), ());
```

### 2. コールバックでトークン取得

```rust
use ne_api::SignInCallback;

// GET /ne_auth/callback?uid=...&state=...  (state の有効期限は 5 分)
let cb = SignInCallback::from_query(query_string).ok_or("uid/state missing")?;
let auth = client.neauth_callback(&cb).await?;

// auth.uid をキーに auth.tokens を保存
my_store_for(&auth.uid).save(&auth.tokens).await?;
```

アプリメニューから起動された場合も、Redirect URI に uid・state が付いて呼ばれるので同じ処理です。

### 3. TokenStore を実装する

```rust
use ne_api::auth::{BoxFuture, TokenStore, Tokens};
use ne_api::BoxError;

struct DbTokenStore { pool: sqlx::PgPool, uid: String }

impl TokenStore for DbTokenStore {
    fn load(&self) -> BoxFuture<'_, Result<Option<Tokens>, BoxError>> {
        Box::pin(async move {
            let row = sqlx::query_as::<_, (String, String)>(
                "SELECT access_token, refresh_token FROM ne_tokens WHERE uid = $1")
                .bind(&self.uid).fetch_optional(&self.pool).await?;
            Ok(row.map(|(a, r)| Tokens::new(a, r)))
        })
    }

    fn save<'a>(&'a self, t: &'a Tokens) -> BoxFuture<'a, Result<(), BoxError>> {
        Box::pin(async move {
            sqlx::query("UPDATE ne_tokens SET access_token = $1, refresh_token = $2 WHERE uid = $3")
                .bind(&t.access_token).bind(&t.refresh_token).bind(&self.uid)
                .execute(&self.pool).await?;
            Ok(())
        })
    }
}

let session = client.session(DbTokenStore { pool, uid });
```

`Tokens` は `Serialize` / `Deserialize` を実装しているので、JSON でそのまま保存することもできます。`Debug` 出力ではトークン値を伏せます。

### バッチ処理

保存済みトークンで `Session` を作って呼ぶだけです。refresh_token の期限（3 日）が切れないよう、2 日以内の間隔で何かしらの API を呼ぶことが推奨されています。高負荷時に待機させる場合は `Client::builder(..).wait_flag(true)` を使います。

### トークンを自分で管理する

```rust
let ex = client.execute(&tokens, "/api_v1_receiveorder_base/count", ()).await;
if let Some(new) = ex.tokens { save(new) }   // エラー時も入ってくることがある
let body = ex.result?;
```

## 使い方

### 検索

```rust
use ne_api::SearchQuery;

let q = SearchQuery::new()
    .fields(["receive_order_id", "receive_order_date", "receive_order_total_amount"])
    .in_("receive_order_order_status_id", ["0", "1", "2", "20"])   // 区分コードは文字列型なので -in を使う
    .eq("receive_order_cancel_type_id", "0")                        // キャンセル以外
    .gte("receive_order_date", "2026-11-01 00:00:00")
    .limit(1000);

let res = session.receiveorder_base().search(&q).await?;   // SearchResponse<Row>
println!("{} 件", res.count);

// 全件（offset を進めて取得）
let all = session.receiveorder_base().search_all(&q).await?;

// 任意の型にデシリアライズ（値は数値項目も文字列で返る点に注意）
#[derive(serde::Deserialize)]
struct Order { receive_order_id: String, receive_order_total_amount: String }
let orders: Vec<Order> = session.receiveorder_base().search_all_as(&q).await?;
```

### 件数・情報取得

```rust
let n = session.receiveorder_base().count(&SearchQuery::new().eq("receive_order_cancel_type_id", "0")).await?;
let statuses = session.system_orderstatus().info().await?;
```

### 更新

```rust
let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<root><receiveorder_base><receive_order_worker_text>メモ</receive_order_worker_text></receiveorder_base></root>"#;

session.receiveorder_base().update([
    ("receive_order_id", "12345"),
    ("receive_order_last_modified_date", "2026-11-06 10:00:00"),
    ("data", xml),
]).await?;
```

### アップロード（非同期）

```rust
let res = session.master_goods().upload([("data_type", "csv"), ("data", csv.as_str())]).await?;
let que = session.system_que()
    .search(&SearchQuery::new().fields(["que_id", "que_status_id", "que_message"]).eq("que_id", res.que_id.as_str()))
    .await?;
```

`data` には `Vec<u8>` も渡せるので、`data_type=gz` の gzip データもそのまま送れます。

### 未対応のパス

```rust
let body = session.request_raw("/api_v1_xxx/yyy", [("key", "value")]).await?;
```

## エラー

```rust
match err {
    ne_api::Error::Api { code, message } => { /* result=error */ }
    ne_api::Error::Redirect { .. } => { /* 再ログインが必要 */ }
    _ => {}
}
err.is_retryable();     // メンテナンス・過負荷など、時間を置けば成功しうるか
err.requires_reauth();  // redirect / 002002 / 002004 / トークン未保存
```

## 動作確認用サンプル

`examples/live_check.rs` は実 API に対してログイン〜検索までを試すサンプルです（ファイルに保存する `TokenStore` の実装例を兼ねます）。認証情報は環境変数から読みます。

```sh
export NE_CLIENT_ID=... NE_CLIENT_SECRET=... NE_REDIRECT_URI=https://localhost:3000/ne_auth/callback
export NE_TOKENS_FILE=/path/to/tokens.json

cargo run --example live_check -- url                      # ログイン URL を表示
cargo run --example live_check -- auth '<リダイレクト先URL>' # uid・state → トークン取得して API 呼び出し
cargo run --example live_check -- check                    # 保存済みトークンで再実行
```

## License

MIT
