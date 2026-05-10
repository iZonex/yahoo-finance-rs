//! Options chain (`v7/finance/options`).

use serde::Deserialize;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// All available expirations for a symbol.
#[derive(Debug, Clone)]
pub struct OptionExpirations {
    /// Symbol echoed back.
    pub symbol: String,
    /// Expirations as UNIX seconds (sorted ascending).
    pub expirations: Vec<i64>,
    /// Strikes available across expirations.
    pub strikes: Vec<f64>,
}

/// One option contract.
#[derive(Debug, Clone, Deserialize)]
pub struct OptionContract {
    /// Yahoo's contract symbol (e.g. `AAPL230120C00150000`).
    #[serde(rename = "contractSymbol")]
    pub contract_symbol: String,
    /// Strike price.
    #[serde(default)]
    pub strike: Option<f64>,
    /// Currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// Last trade price.
    #[serde(default, rename = "lastPrice")]
    pub last_price: Option<f64>,
    /// Change since prior close.
    #[serde(default)]
    pub change: Option<f64>,
    /// Percent change since prior close.
    #[serde(default, rename = "percentChange")]
    pub percent_change: Option<f64>,
    /// Trading volume.
    #[serde(default)]
    pub volume: Option<u64>,
    /// Open interest.
    #[serde(default, rename = "openInterest")]
    pub open_interest: Option<u64>,
    /// Bid price.
    #[serde(default)]
    pub bid: Option<f64>,
    /// Ask price.
    #[serde(default)]
    pub ask: Option<f64>,
    /// Contract size (`REGULAR`).
    #[serde(default, rename = "contractSize")]
    pub contract_size: Option<String>,
    /// Expiration time (UNIX seconds).
    #[serde(default)]
    pub expiration: Option<i64>,
    /// Last trade time (UNIX seconds).
    #[serde(default, rename = "lastTradeDate")]
    pub last_trade_date: Option<i64>,
    /// Implied volatility (decimal).
    #[serde(default, rename = "impliedVolatility")]
    pub implied_volatility: Option<f64>,
    /// Whether the contract is in-the-money.
    #[serde(default, rename = "inTheMoney")]
    pub in_the_money: Option<bool>,
}

/// Calls and puts for a single expiration.
#[derive(Debug, Clone)]
pub struct OptionChain {
    /// Underlying symbol.
    pub symbol: String,
    /// The expiration this chain is for (UNIX seconds).
    pub expiration: Option<i64>,
    /// Call contracts.
    pub calls: Vec<OptionContract>,
    /// Put contracts.
    pub puts: Vec<OptionContract>,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "optionChain")]
    option_chain: OptionChainEnv,
}

#[derive(Debug, Deserialize)]
struct OptionChainEnv {
    #[serde(default)]
    result: Vec<OptionResult>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OptionResult {
    #[serde(rename = "underlyingSymbol", default)]
    underlying_symbol: Option<String>,
    #[serde(rename = "expirationDates", default)]
    expiration_dates: Vec<i64>,
    #[serde(default)]
    strikes: Vec<f64>,
    #[serde(default)]
    options: Vec<OptionsBlock>,
}

#[derive(Debug, Deserialize)]
struct OptionsBlock {
    #[serde(default, rename = "expirationDate")]
    expiration_date: Option<i64>,
    #[serde(default)]
    calls: Vec<OptionContract>,
    #[serde(default)]
    puts: Vec<OptionContract>,
}

impl OptionExpirations {
    pub(crate) async fn fetch(client: &YfClient, symbol: &str) -> Result<Self> {
        let path = format!(
            "/v7/finance/options/{}",
            crate::info::history_percent(symbol)
        );
        let env: Envelope = client.get_json(&path, &[]).await?;
        check_error(&env, symbol)?;
        let r = env
            .option_chain
            .result
            .into_iter()
            .next()
            .ok_or_else(|| Error::TickerMissing {
                ticker: symbol.to_string(),
                reason: "options endpoint returned no result".into(),
            })?;
        Ok(OptionExpirations {
            symbol: r.underlying_symbol.unwrap_or_else(|| symbol.to_string()),
            expirations: r.expiration_dates,
            strikes: r.strikes,
        })
    }
}

impl OptionChain {
    pub(crate) async fn fetch(
        client: &YfClient,
        symbol: &str,
        expiration: Option<i64>,
    ) -> Result<Self> {
        let path = format!(
            "/v7/finance/options/{}",
            crate::info::history_percent(symbol)
        );
        let q = match expiration {
            Some(d) => vec![("date", d.to_string())],
            None => vec![],
        };
        let env: Envelope = client.get_json(&path, &q).await?;
        check_error(&env, symbol)?;
        let r = env
            .option_chain
            .result
            .into_iter()
            .next()
            .ok_or_else(|| Error::TickerMissing {
                ticker: symbol.to_string(),
                reason: "options endpoint returned no result".into(),
            })?;
        let block = r.options.into_iter().next().unwrap_or(OptionsBlock {
            expiration_date: None,
            calls: vec![],
            puts: vec![],
        });
        Ok(OptionChain {
            symbol: r.underlying_symbol.unwrap_or_else(|| symbol.to_string()),
            expiration: block.expiration_date.or(expiration),
            calls: block.calls,
            puts: block.puts,
        })
    }
}

fn check_error(env: &Envelope, symbol: &str) -> Result<()> {
    if let Some(err) = &env.option_chain.error {
        return Err(Error::Yahoo {
            symbol: symbol.to_string(),
            code: "options_error".into(),
            description: err.to_string(),
        });
    }
    Ok(())
}
