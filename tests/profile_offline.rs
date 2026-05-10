//! Offline replay for `Ticker::profile`.

mod common;

use yfinance::{Profile, Ticker};

#[tokio::test]
async fn offline_profile_uses_recorded_fixture() {
    let symbol = "AAPL";
    if !common::fixture_exists("profile_quoteSummary", symbol, "json") {
        eprintln!(
            "skipping — record the fixture first with \
             `just test-record offline_profile_uses_recorded_fixture`"
        );
        return;
    }

    let server = common::setup_server();
    let _cookie_crumb = common::mock_cookie_crumb(&server);
    let mock = common::mock_quote_summary(&server, symbol, "profile_quoteSummary");
    let client = common::build_test_client(&server);

    let profile = Ticker::new(&client, symbol)
        .profile()
        .await
        .expect("profile parse");
    mock.assert();
    match profile {
        Profile::Company(c) => assert!(!c.name.is_empty()),
        Profile::Fund(f) => assert!(!f.name.is_empty()),
        Profile::Other { name, .. } => assert!(!name.is_empty()),
    }
}
