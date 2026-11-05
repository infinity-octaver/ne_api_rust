//! Typed accessors for every documented endpoint, generated from
//! <https://developer.next-engine.com/llms-full.txt>.
//!
//! Each group is obtained from a [`Session`], e.g.
//! `session.receiveorder_base()`. Operations:
//!
//! * `count(&SearchQuery) -> u64`
//! * `search(&SearchQuery)` / `search_as::<T>(..)` / `search_all(..)` / `search_all_as::<T>(..)`
//! * `info()` / `info_as::<T>()` / `info_with(params)` — returns every row, no conditions
//! * `upload(params) -> UploadResponse` — asynchronous; check the queue with `system_que()`
//! * other actions (`update`, `shipped`, `create`, …) take [`Params`] and return the raw JSON object.
//!
//! Endpoint paths not covered here can be called with [`Session::request_raw`].

#![allow(clippy::needless_lifetimes)]

use serde::de::DeserializeOwned;

use crate::error::Result;
use crate::params::Params;
use crate::query::SearchQuery;
use crate::response::{Row, SearchResponse, UploadResponse};
use crate::session::Session;

macro_rules! op {
    ($path:literal, count) => {
        /// 件数取得 (`/count`).
        pub async fn count(&self, query: &SearchQuery) -> Result<u64> {
            self.session.count(concat!($path, "/count"), query).await
        }
    };
    ($path:literal, search) => {
        /// 検索 (`/search`). `fields` is required.
        pub async fn search(&self, query: &SearchQuery) -> Result<SearchResponse<Row>> {
            self.session
                .search_as(concat!($path, "/search"), query)
                .await
        }
        /// 検索 (`/search`) with rows deserialized into `T`.
        pub async fn search_as<T: DeserializeOwned>(
            &self,
            query: &SearchQuery,
        ) -> Result<SearchResponse<T>> {
            self.session
                .search_as(concat!($path, "/search"), query)
                .await
        }
        /// All pages of [`search`](Self::search). See [`Session::search_all_as`].
        pub async fn search_all(&self, query: &SearchQuery) -> Result<Vec<Row>> {
            self.session
                .search_all_as(concat!($path, "/search"), query)
                .await
        }
        /// All pages of [`search_as`](Self::search_as). See [`Session::search_all_as`].
        pub async fn search_all_as<T: DeserializeOwned>(
            &self,
            query: &SearchQuery,
        ) -> Result<Vec<T>> {
            self.session
                .search_all_as(concat!($path, "/search"), query)
                .await
        }
    };
    ($path:literal, info) => {
        /// 情報取得 (`/info`): all rows, all fields.
        pub async fn info(&self) -> Result<SearchResponse<Row>> {
            self.session
                .request(concat!($path, "/info"), Params::new())
                .await
        }
        /// 情報取得 (`/info`) with rows deserialized into `T`.
        pub async fn info_as<T: DeserializeOwned>(&self) -> Result<SearchResponse<T>> {
            self.session
                .request(concat!($path, "/info"), Params::new())
                .await
        }
        /// 情報取得 (`/info`) with extra parameters.
        pub async fn info_with<T: DeserializeOwned>(
            &self,
            params: impl Into<Params>,
        ) -> Result<SearchResponse<T>> {
            self.session.request(concat!($path, "/info"), params).await
        }
    };
    ($path:literal, upload) => {
        /// アップロード (`/upload`). Registered asynchronously; returns the upload queue id.
        pub async fn upload(&self, params: impl Into<Params>) -> Result<UploadResponse> {
            self.session
                .upload_at(concat!($path, "/upload"), params)
                .await
        }
    };
    ($path:literal, $action:ident) => {
        #[doc = concat!("`", $path, "/", stringify!($action), "`. Returns the raw JSON object.")]
        pub async fn $action(&self, params: impl Into<Params>) -> Result<Row> {
            self.session
                .request_raw(concat!($path, "/", stringify!($action)), params)
                .await
        }
    };
}

macro_rules! resource {
    ($(#[$meta:meta])* $method:ident => $ty:ident, $path:literal, [$($op:ident),* $(,)?]) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy)]
        pub struct $ty<'a> {
            session: &'a Session,
        }

        impl<'a> $ty<'a> {
            /// Base path of this endpoint group.
            pub const PATH: &'static str = $path;
            $( op!($path, $op); )*
        }

        impl Session {
            #[doc = concat!("Endpoints under `", $path, "`.")]
            pub fn $method(&self) -> $ty<'_> {
                $ty { session: self }
            }
        }
    };
}

resource! {
    /// `/api_v1_login_company` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 企業情報取得
    login_company => LoginCompany, "/api_v1_login_company", [info]
}

resource! {
    /// `/api_v1_login_user` のエンドポイント群。
    ///
    /// - [`info`](Self::info): ログインユーザー情報取得
    login_user => LoginUser, "/api_v1_login_user", [info]
}

resource! {
    /// `/api_v1_master_goods` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品マスタ件数取得
    /// - [`search`](Self::search): 商品マスタ検索
    /// - [`upload`](Self::upload): 商品マスタアップロード
    master_goods => MasterGoods, "/api_v1_master_goods", [count, search, upload]
}

resource! {
    /// `/api_v1_master_goods_page` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品ページ件数取得
    /// - [`search`](Self::search): 商品ページ検索
    master_goods_page => MasterGoodsPage, "/api_v1_master_goods_page", [count, search]
}

resource! {
    /// `/api_v1_master_goods_page_option` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品ページ(オプション)件数取得
    /// - [`search`](Self::search): 商品ページ(オプション)検索
    master_goods_page_option => MasterGoodsPageOption, "/api_v1_master_goods_page_option", [count, search]
}

resource! {
    /// `/api_v1_master_goods_page_shopcategory` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品ページ(カテゴリ)件数取得
    /// - [`search`](Self::search): 商品ページ(カテゴリ)検索
    master_goods_page_shopcategory => MasterGoodsPageShopcategory, "/api_v1_master_goods_page_shopcategory", [count, search]
}

resource! {
    /// `/api_v1_master_goods_page_variation` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品ページ(バリエーション)件数取得
    /// - [`search`](Self::search): 商品ページ(バリエーション)検索
    master_goods_page_variation => MasterGoodsPageVariation, "/api_v1_master_goods_page_variation", [count, search]
}

resource! {
    /// `/api_v1_master_goodscategory` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品カテゴリ件数取得
    /// - [`search`](Self::search): 商品カテゴリ検索
    master_goodscategory => MasterGoodscategory, "/api_v1_master_goodscategory", [count, search]
}

resource! {
    /// `/api_v1_master_goodsimage` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品画像件数取得
    /// - [`search`](Self::search): 商品画像検索
    master_goodsimage => MasterGoodsimage, "/api_v1_master_goodsimage", [count, search]
}

resource! {
    /// `/api_v1_master_goodsimagetag` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品画像分類タグ件数取得
    /// - [`search`](Self::search): 商品画像分類タグ検索
    master_goodsimagetag => MasterGoodsimagetag, "/api_v1_master_goodsimagetag", [count, search]
}

resource! {
    /// `/api_v1_master_goodsmapping` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品コード紐づけ件数取得
    /// - [`search`](Self::search): 商品コード紐づけ検索
    master_goodsmapping => MasterGoodsmapping, "/api_v1_master_goodsmapping", [count, search]
}

resource! {
    /// `/api_v1_master_goodstag` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品タグ件数取得
    /// - [`search`](Self::search): 商品タグ検索
    master_goodstag => MasterGoodstag, "/api_v1_master_goodstag", [count, search]
}

resource! {
    /// `/api_v1_master_goodswholesale` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 商品別卸先マスタ件数取得
    /// - [`search`](Self::search): 商品別卸先マスタ検索
    master_goodswholesale => MasterGoodswholesale, "/api_v1_master_goodswholesale", [count, search]
}

resource! {
    /// `/api_v1_master_keepstock` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 区分け在庫マスタ件数取得
    /// - [`search`](Self::search): 区分け在庫マスタ検索
    master_keepstock => MasterKeepstock, "/api_v1_master_keepstock", [count, search]
}

resource! {
    /// `/api_v1_master_mailtag` のエンドポイント群。
    ///
    /// - [`count`](Self::count): メールタグ件数取得
    /// - [`search`](Self::search): メールタグ検索
    master_mailtag => MasterMailtag, "/api_v1_master_mailtag", [count, search]
}

resource! {
    /// `/api_v1_master_setgoods` のエンドポイント群。
    ///
    /// - [`count`](Self::count): セット商品マスタ件数取得
    /// - [`search`](Self::search): セット商品マスタ検索
    /// - [`upload`](Self::upload): セット商品マスタアップロード
    master_setgoods => MasterSetgoods, "/api_v1_master_setgoods", [count, search, upload]
}

resource! {
    /// `/api_v1_master_setstock` のエンドポイント群。
    ///
    /// - [`count`](Self::count): セット在庫マスタ件数取得
    /// - [`search`](Self::search): セット在庫マスタ検索
    master_setstock => MasterSetstock, "/api_v1_master_setstock", [count, search]
}

resource! {
    /// `/api_v1_master_shop` のエンドポイント群。
    ///
    /// - [`checkconnect`](Self::checkconnect): 店舗マスタ店舗接続確認
    /// - [`count`](Self::count): 店舗マスタ件数取得
    /// - [`create`](Self::create): 店舗マスタ店舗作成
    /// - [`mailaddress`](Self::mailaddress): 店舗マスタ受注情報取り込み用メールアドレス取得
    /// - [`search`](Self::search): 店舗マスタ検索
    /// - [`update`](Self::update): 店舗マスタ店舗更新
    master_shop => MasterShop, "/api_v1_master_shop", [checkconnect, count, create, mailaddress, search, update]
}

resource! {
    /// `/api_v1_master_stock` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 在庫マスタ件数取得
    /// - [`search`](Self::search): 在庫マスタ検索
    master_stock => MasterStock, "/api_v1_master_stock", [count, search]
}

resource! {
    /// `/api_v1_master_stockiohistory` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 在庫入出庫履歴件数取得
    /// - [`search`](Self::search): 在庫入出庫履歴検索
    master_stockiohistory => MasterStockiohistory, "/api_v1_master_stockiohistory", [count, search]
}

resource! {
    /// `/api_v1_master_supplier` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 仕入先マスタ件数取得
    /// - [`search`](Self::search): 仕入先マスタ検索
    master_supplier => MasterSupplier, "/api_v1_master_supplier", [count, search]
}

resource! {
    /// `/api_v1_master_wholesale` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 卸先マスタ件数取得
    /// - [`search`](Self::search): 卸先マスタ検索
    master_wholesale => MasterWholesale, "/api_v1_master_wholesale", [count, search]
}

resource! {
    /// `/api_v1_notice_execution` のエンドポイント群。
    ///
    /// - [`add`](Self::add): 実行結果お知らせの追加
    /// - [`count`](Self::count): 実行結果お知らせ件数取得
    /// - [`search`](Self::search): 実行結果お知らせ検索
    notice_execution => NoticeExecution, "/api_v1_notice_execution", [add, count, search]
}

resource! {
    /// `/api_v1_placeorder_base` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 発注伝票件数取得
    /// - [`search`](Self::search): 発注伝票検索
    placeorder_base => PlaceorderBase, "/api_v1_placeorder_base", [count, search]
}

resource! {
    /// `/api_v1_placeorder_row` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 発注明細件数取得
    /// - [`search`](Self::search): 発注明細検索
    placeorder_row => PlaceorderRow, "/api_v1_placeorder_row", [count, search]
}

resource! {
    /// `/api_v1_receiveorder_base` のエンドポイント群。
    ///
    /// - [`bulkupdate`](Self::bulkupdate): 受注伝票一括更新
    /// - [`bulkupdatereceipted`](Self::bulkupdatereceipted): 納品書印刷済み一括更新
    /// - [`bundle`](Self::bundle): 受注伝票一括同梱
    /// - [`bundle_candidate_groups`](Self::bundle_candidate_groups): 受注伝票 同梱候補グループ取得
    /// - [`count`](Self::count): 受注伝票件数取得
    /// - [`divide`](Self::divide): 受注伝票分割
    /// - [`labelprinted`](Self::labelprinted): 受注伝票配送情報出力済み
    /// - [`receipted`](Self::receipted): 受注伝票納品書印刷済み
    /// - [`search`](Self::search): 受注伝票検索
    /// - [`shipped`](Self::shipped): 受注伝票出荷確定処理
    /// - [`update`](Self::update): 受注伝票更新
    /// - [`upload`](Self::upload): 受注伝票アップロード
    receiveorder_base => ReceiveorderBase, "/api_v1_receiveorder_base", [bulkupdate, bulkupdatereceipted, bundle, bundle_candidate_groups, count, divide, labelprinted, receipted, search, shipped, update, upload]
}

resource! {
    /// `/api_v1_receiveorder_confirm` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 受注確認内容件数取得
    /// - [`search`](Self::search): 受注確認内容検索
    /// - [`update`](Self::update): 受注確認内容更新
    receiveorder_confirm => ReceiveorderConfirm, "/api_v1_receiveorder_confirm", [count, search, update]
}

resource! {
    /// `/api_v1_receiveorder_forwardingagent` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 発送方法別区分件数取得
    /// - [`search`](Self::search): 発送方法別区分検索
    receiveorder_forwardingagent => ReceiveorderForwardingagent, "/api_v1_receiveorder_forwardingagent", [count, search]
}

resource! {
    /// `/api_v1_receiveorder_groupingtag` のエンドポイント群。
    ///
    /// - [`search`](Self::search): 受注分類タグ検索
    receiveorder_groupingtag => ReceiveorderGroupingtag, "/api_v1_receiveorder_groupingtag", [search]
}

resource! {
    /// `/api_v1_receiveorder_option` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 受注伝票オプション件数取得
    /// - [`search`](Self::search): 受注伝票オプション検索
    receiveorder_option => ReceiveorderOption, "/api_v1_receiveorder_option", [count, search]
}

resource! {
    /// `/api_v1_receiveorder_paymentdeliveryconvert` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 支払発送変換件数取得
    /// - [`search`](Self::search): 支払発送変換検索
    receiveorder_paymentdeliveryconvert => ReceiveorderPaymentdeliveryconvert, "/api_v1_receiveorder_paymentdeliveryconvert", [count, search]
}

resource! {
    /// `/api_v1_receiveorder_row` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 受注明細件数取得
    /// - [`search`](Self::search): 受注明細検索
    receiveorder_row => ReceiveorderRow, "/api_v1_receiveorder_row", [count, search]
}

resource! {
    /// `/api_v1_receiveorder_tax_account` のエンドポイント群。
    ///
    /// - [`search`](Self::search): 受注税金内訳取得
    receiveorder_tax_account => ReceiveorderTaxAccount, "/api_v1_receiveorder_tax_account", [search]
}

resource! {
    /// `/api_v1_receiveorder_uploadpattern` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 受注一括登録パターン情報取得
    receiveorder_uploadpattern => ReceiveorderUploadpattern, "/api_v1_receiveorder_uploadpattern", [info]
}

resource! {
    /// `/api_v1_supplyorder_base` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 仕入伝票件数取得
    /// - [`search`](Self::search): 仕入伝票検索
    supplyorder_base => SupplyorderBase, "/api_v1_supplyorder_base", [count, search]
}

resource! {
    /// `/api_v1_supplyorder_row` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 仕入明細件数取得
    /// - [`search`](Self::search): 仕入明細検索
    supplyorder_row => SupplyorderRow, "/api_v1_supplyorder_row", [count, search]
}

resource! {
    /// `/api_v1_system_authorizationtype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): オーソリ区分情報取得
    system_authorizationtype => SystemAuthorizationtype, "/api_v1_system_authorizationtype", [info]
}

resource! {
    /// `/api_v1_system_canceltype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 受注キャンセル区分情報取得
    system_canceltype => SystemCanceltype, "/api_v1_system_canceltype", [info]
}

resource! {
    /// `/api_v1_system_confirmcheck` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 確認チェック区分情報取得
    system_confirmcheck => SystemConfirmcheck, "/api_v1_system_confirmcheck", [info]
}

resource! {
    /// `/api_v1_system_creditapprovaltype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): クレジット承認区分情報取得
    system_creditapprovaltype => SystemCreditapprovaltype, "/api_v1_system_creditapprovaltype", [info]
}

resource! {
    /// `/api_v1_system_creditauthorizationcenter` のエンドポイント群。
    ///
    /// - [`info`](Self::info): クレジットオーソリセンター区分情報取得
    system_creditauthorizationcenter => SystemCreditauthorizationcenter, "/api_v1_system_creditauthorizationcenter", [info]
}

resource! {
    /// `/api_v1_system_credittype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): クレジット種類区分情報取得
    system_credittype => SystemCredittype, "/api_v1_system_credittype", [info]
}

resource! {
    /// `/api_v1_system_currencyunit` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 通貨単位区分情報取得
    system_currencyunit => SystemCurrencyunit, "/api_v1_system_currencyunit", [info]
}

resource! {
    /// `/api_v1_system_customertype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 顧客区分情報取得
    system_customertype => SystemCustomertype, "/api_v1_system_customertype", [info]
}

resource! {
    /// `/api_v1_system_delivery` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 発送方法区分情報取得
    system_delivery => SystemDelivery, "/api_v1_system_delivery", [info]
}

resource! {
    /// `/api_v1_system_deliverydate` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 納期情報取得
    system_deliverydate => SystemDeliverydate, "/api_v1_system_deliverydate", [info]
}

resource! {
    /// `/api_v1_system_deposittype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 入金区分情報取得
    system_deposittype => SystemDeposittype, "/api_v1_system_deposittype", [info]
}

resource! {
    /// `/api_v1_system_forwardingmethod` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 輸送区分情報取得
    system_forwardingmethod => SystemForwardingmethod, "/api_v1_system_forwardingmethod", [info]
}

resource! {
    /// `/api_v1_system_fraction` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 端数処理区分情報取得
    system_fraction => SystemFraction, "/api_v1_system_fraction", [info]
}

resource! {
    /// `/api_v1_system_goodsstatus` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 商品ステータス区分情報取得
    system_goodsstatus => SystemGoodsstatus, "/api_v1_system_goodsstatus", [info]
}

resource! {
    /// `/api_v1_system_goodstype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 商品区分情報取得
    system_goodstype => SystemGoodstype, "/api_v1_system_goodstype", [info]
}

resource! {
    /// `/api_v1_system_imagetag` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 画像分類タグ件数取得
    /// - [`search`](Self::search): 画像分類タグ検索
    system_imagetag => SystemImagetag, "/api_v1_system_imagetag", [count, search]
}

resource! {
    /// `/api_v1_system_importantcheck` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 重要チェック区分情報取得
    system_importantcheck => SystemImportantcheck, "/api_v1_system_importantcheck", [info]
}

resource! {
    /// `/api_v1_system_importtype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 取込種類区分情報取得
    system_importtype => SystemImporttype, "/api_v1_system_importtype", [info]
}

resource! {
    /// `/api_v1_system_iotype` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 入出荷区分情報取得
    system_iotype => SystemIotype, "/api_v1_system_iotype", [info]
}

resource! {
    /// `/api_v1_system_itemname` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 項目名情報取得
    system_itemname => SystemItemname, "/api_v1_system_itemname", [info]
}

resource! {
    /// `/api_v1_system_mall` のエンドポイント群。
    ///
    /// - [`count`](Self::count): モール件数取得
    /// - [`search`](Self::search): モール検索
    system_mall => SystemMall, "/api_v1_system_mall", [count, search]
}

resource! {
    /// `/api_v1_system_mallcategory` のエンドポイント群。
    ///
    /// - [`count`](Self::count): モールカテゴリ件数取得
    /// - [`search`](Self::search): モールカテゴリ検索
    system_mallcategory => SystemMallcategory, "/api_v1_system_mallcategory", [count, search]
}

resource! {
    /// `/api_v1_system_merchandise` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 取扱区分情報取得
    system_merchandise => SystemMerchandise, "/api_v1_system_merchandise", [info]
}

resource! {
    /// `/api_v1_system_order` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 発注区分情報取得
    system_order => SystemOrder, "/api_v1_system_order", [info]
}

resource! {
    /// `/api_v1_system_ordercondition` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 発注条件区分情報取得
    system_ordercondition => SystemOrdercondition, "/api_v1_system_ordercondition", [info]
}

resource! {
    /// `/api_v1_system_orderstatus` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 受注状態区分情報取得
    system_orderstatus => SystemOrderstatus, "/api_v1_system_orderstatus", [info]
}

resource! {
    /// `/api_v1_system_pagestatus` のエンドポイント群。
    ///
    /// - [`info`](Self::info): ページステータス区分情報取得
    system_pagestatus => SystemPagestatus, "/api_v1_system_pagestatus", [info]
}

resource! {
    /// `/api_v1_system_paymentmethod` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 支払区分情報取得
    system_paymentmethod => SystemPaymentmethod, "/api_v1_system_paymentmethod", [info]
}

resource! {
    /// `/api_v1_system_payout` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 支払方法区分情報取得
    system_payout => SystemPayout, "/api_v1_system_payout", [info]
}

resource! {
    /// `/api_v1_system_que` のエンドポイント群。
    ///
    /// - [`count`](Self::count): アップロードキュー件数取得
    /// - [`search`](Self::search): アップロードキュー検索
    system_que => SystemQue, "/api_v1_system_que", [count, search]
}

resource! {
    /// `/api_v1_system_returnedreason` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 返品事由区分情報取得
    system_returnedreason => SystemReturnedreason, "/api_v1_system_returnedreason", [info]
}

resource! {
    /// `/api_v1_system_select` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 商品選択肢区分情報取得
    system_select => SystemSelect, "/api_v1_system_select", [info]
}

resource! {
    /// `/api_v1_system_socialinsurance` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 社会保険区分情報取得
    system_socialinsurance => SystemSocialinsurance, "/api_v1_system_socialinsurance", [info]
}

resource! {
    /// `/api_v1_system_tax` のエンドポイント群。
    ///
    /// - [`info`](Self::info): 税区分情報取得
    system_tax => SystemTax, "/api_v1_system_tax", [info]
}

resource! {
    /// `/api_v1_warehouse_base` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 拠点マスタ件数取得
    /// - [`search`](Self::search): 拠点マスタ検索
    warehouse_base => WarehouseBase, "/api_v1_warehouse_base", [count, search]
}

resource! {
    /// `/api_v1_warehouse_stock` のエンドポイント群。
    ///
    /// - [`count`](Self::count): 拠点在庫マスタ件数取得
    /// - [`search`](Self::search): 拠点在庫マスタ検索
    /// - [`upload`](Self::upload): 拠点在庫マスタアップロード
    warehouse_stock => WarehouseStock, "/api_v1_warehouse_stock", [count, search, upload]
}
