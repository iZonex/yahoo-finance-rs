//! Financial statements via the timeseries endpoint.
//!
//! Mirrors `yfinance/scrapers/fundamentals.py`. Fetches up to 4 annual or 5
//! quarterly periods of income / balance sheet / cash flow data.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;

use crate::client::YfClient;
use crate::error::{Error, Result};

/// Which financial statement to request.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FundamentalsKind {
    /// Income statement (revenue, net income, EPS, …).
    #[default]
    IncomeStatement,
    /// Balance sheet (assets, liabilities, equity).
    BalanceSheet,
    /// Cash flow statement.
    CashFlow,
}

impl FundamentalsKind {
    /// Metric names sent to the timeseries endpoint, **without** the
    /// `annual` / `quarterly` prefix.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            FundamentalsKind::IncomeStatement => INCOME_KEYS,
            FundamentalsKind::BalanceSheet => BALANCE_KEYS,
            FundamentalsKind::CashFlow => CASHFLOW_KEYS,
        }
    }
}

/// A `metric -> { date -> value }` view of one financial statement.
#[derive(Debug, Clone, Default)]
pub struct Fundamentals {
    /// Symbol echoed back.
    pub symbol: String,
    /// Income statement, balance sheet, or cash flow.
    pub kind: FundamentalsKind,
    /// Whether quarterly (`true`) or annual (`false`).
    pub quarterly: bool,
    /// `metric_name -> sorted_date_to_value`.
    pub series: BTreeMap<String, BTreeMap<NaiveDate, f64>>,
}

impl Fundamentals {
    /// All distinct dates in the response, sorted oldest -> newest.
    pub fn dates(&self) -> Vec<NaiveDate> {
        let mut set: std::collections::BTreeSet<NaiveDate> = std::collections::BTreeSet::new();
        for s in self.series.values() {
            for d in s.keys() {
                set.insert(*d);
            }
        }
        set.into_iter().collect()
    }

    /// Look up a single value.
    pub fn value(&self, metric: &str, date: NaiveDate) -> Option<f64> {
        self.series.get(metric).and_then(|s| s.get(&date)).copied()
    }

    pub(crate) async fn fetch(
        client: &YfClient,
        symbol: &str,
        kind: FundamentalsKind,
        quarterly: bool,
    ) -> Result<Self> {
        let prefix = if quarterly { "quarterly" } else { "annual" };
        let typ_param = kind
            .keys()
            .iter()
            .map(|k| format!("{prefix}{k}"))
            .collect::<Vec<_>>()
            .join(",");

        // Yahoo always wants a wide window; mirroring the Python lib.
        let start = "493590046".to_string(); // 1985-08-14, far enough back
        let end = chrono::Utc::now().timestamp().to_string();

        let path = format!(
            "/ws/fundamentals-timeseries/v1/finance/timeseries/{}",
            crate::info::history_percent(symbol)
        );
        let q = vec![
            ("symbol", symbol.to_string()),
            ("type", typ_param),
            ("period1", start),
            ("period2", end),
            ("merge", "false".into()),
            ("padTimeSeries", "true".into()),
            ("lang", "en-US".into()),
            ("region", "US".into()),
        ];

        let raw: TimeseriesEnvelope = client.get_json_crumb(&path, &q).await?;
        if let Some(err) = raw.timeseries.error {
            return Err(Error::Yahoo {
                symbol: symbol.to_string(),
                code: "timeseries_error".into(),
                description: err.to_string(),
            });
        }

        let mut series: BTreeMap<String, BTreeMap<NaiveDate, f64>> = BTreeMap::new();
        for entry in raw.timeseries.result {
            for (k, v) in entry.iter() {
                if k == "meta" || k == "timestamp" {
                    continue;
                }
                let arr = match v {
                    Value::Array(a) => a,
                    _ => continue,
                };
                let metric = strip_prefix(k, prefix).to_string();
                let map = series.entry(metric).or_default();
                for item in arr {
                    let Some(item) = item.as_object() else {
                        continue;
                    };
                    let Some(as_of) = item.get("asOfDate").and_then(|s| s.as_str()) else {
                        continue;
                    };
                    let Ok(date) = NaiveDate::parse_from_str(as_of, "%Y-%m-%d") else {
                        continue;
                    };
                    let value = item
                        .get("reportedValue")
                        .and_then(|rv| rv.get("raw"))
                        .and_then(|x| x.as_f64());
                    if let Some(v) = value {
                        map.insert(date, v);
                    }
                }
            }
        }

        Ok(Fundamentals {
            symbol: symbol.to_string(),
            kind,
            quarterly,
            series,
        })
    }
}

fn strip_prefix<'a>(key: &'a str, prefix: &str) -> &'a str {
    key.strip_prefix(prefix).unwrap_or(key)
}

#[derive(Debug, Deserialize)]
struct TimeseriesEnvelope {
    timeseries: TimeseriesContent,
}

#[derive(Debug, Deserialize)]
struct TimeseriesContent {
    #[serde(default)]
    result: Vec<serde_json::Map<String, Value>>,
    #[serde(default)]
    error: Option<Value>,
}

// ---------------- key tables ----------------
//
// Comprehensive lists ported from `yfinance`'s upstream `const.py`. Yahoo's
// timeseries endpoint silently drops keys it doesn't recognize for a given
// symbol, so requesting all of them is the same cost as requesting a subset.

const INCOME_KEYS: &[&str] = &[
    "TaxEffectOfUnusualItems",
    "TaxRateForCalcs",
    "NormalizedEBITDA",
    "NormalizedDilutedEPS",
    "NormalizedBasicEPS",
    "TotalUnusualItems",
    "TotalUnusualItemsExcludingGoodwill",
    "NetIncomeFromContinuingOperationNetMinorityInterest",
    "ReconciledDepreciation",
    "ReconciledCostOfRevenue",
    "EBITDA",
    "EBIT",
    "NetInterestIncome",
    "InterestExpense",
    "InterestIncome",
    "ContinuingAndDiscontinuedDilutedEPS",
    "ContinuingAndDiscontinuedBasicEPS",
    "NormalizedIncome",
    "NetIncomeFromContinuingAndDiscontinuedOperation",
    "TotalExpenses",
    "RentExpenseSupplemental",
    "ReportedNormalizedDilutedEPS",
    "ReportedNormalizedBasicEPS",
    "TotalOperatingIncomeAsReported",
    "DividendPerShare",
    "DilutedAverageShares",
    "BasicAverageShares",
    "DilutedEPS",
    "BasicEPS",
    "DilutedNIAvailtoComStockholders",
    "AverageDilutionEarnings",
    "NetIncomeCommonStockholders",
    "PreferredStockDividends",
    "NetIncome",
    "MinorityInterests",
    "NetIncomeIncludingNoncontrollingInterests",
    "NetIncomeContinuousOperations",
    "TaxProvision",
    "PretaxIncome",
    "OtherIncomeExpense",
    "OtherNonOperatingIncomeExpenses",
    "SpecialIncomeCharges",
    "GainOnSaleOfPPE",
    "GainOnSaleOfBusiness",
    "OtherSpecialCharges",
    "WriteOff",
    "ImpairmentOfCapitalAssets",
    "RestructuringAndMergernAcquisition",
    "EarningsFromEquityInterest",
    "GainOnSaleOfSecurity",
    "NetNonOperatingInterestIncomeExpense",
    "TotalOtherFinanceCost",
    "InterestExpenseNonOperating",
    "InterestIncomeNonOperating",
    "OperatingIncome",
    "OperatingExpense",
    "OtherOperatingExpenses",
    "OtherTaxes",
    "ProvisionForDoubtfulAccounts",
    "DepreciationAmortizationDepletionIncomeStatement",
    "DepreciationAndAmortizationInIncomeStatement",
    "Amortization",
    "AmortizationOfIntangiblesIncomeStatement",
    "DepreciationIncomeStatement",
    "ResearchAndDevelopment",
    "SellingGeneralAndAdministration",
    "SellingAndMarketingExpense",
    "GeneralAndAdministrativeExpense",
    "OtherGandA",
    "InsuranceAndClaims",
    "RentAndLandingFees",
    "SalariesAndWages",
    "GrossProfit",
    "CostOfRevenue",
    "TotalRevenue",
    "ExciseTaxes",
    "OperatingRevenue",
    "OccupancyAndEquipment",
    "OtherNonInterestExpense",
];

const BALANCE_KEYS: &[&str] = &[
    "TreasurySharesNumber",
    "PreferredSharesNumber",
    "OrdinarySharesNumber",
    "ShareIssued",
    "NetDebt",
    "TotalDebt",
    "TangibleBookValue",
    "InvestedCapital",
    "WorkingCapital",
    "NetTangibleAssets",
    "CapitalLeaseObligations",
    "CommonStockEquity",
    "PreferredStockEquity",
    "TotalCapitalization",
    "TotalEquityGrossMinorityInterest",
    "MinorityInterest",
    "StockholdersEquity",
    "OtherEquityInterest",
    "GainsLossesNotAffectingRetainedEarnings",
    "OtherEquityAdjustments",
    "ForeignCurrencyTranslationAdjustments",
    "MinimumPensionLiabilities",
    "UnrealizedGainLoss",
    "TreasuryStock",
    "RetainedEarnings",
    "AdditionalPaidInCapital",
    "CapitalStock",
    "CommonStock",
    "PreferredStock",
    "TotalLiabilitiesNetMinorityInterest",
    "TotalNonCurrentLiabilitiesNetMinorityInterest",
    "OtherNonCurrentLiabilities",
    "LiabilitiesHeldforSaleNonCurrent",
    "EmployeeBenefits",
    "NonCurrentPensionAndOtherPostretirementBenefitPlans",
    "NonCurrentAccruedExpenses",
    "TradeandOtherPayablesNonCurrent",
    "NonCurrentDeferredLiabilities",
    "NonCurrentDeferredRevenue",
    "NonCurrentDeferredTaxesLiabilities",
    "LongTermDebtAndCapitalLeaseObligation",
    "LongTermCapitalLeaseObligation",
    "LongTermDebt",
    "LongTermProvisions",
    "CurrentLiabilities",
    "OtherCurrentLiabilities",
    "CurrentDeferredLiabilities",
    "CurrentDeferredRevenue",
    "CurrentDeferredTaxesLiabilities",
    "CurrentDebtAndCapitalLeaseObligation",
    "CurrentCapitalLeaseObligation",
    "CurrentDebt",
    "OtherCurrentBorrowings",
    "LineOfCredit",
    "CommercialPaper",
    "CurrentNotesPayable",
    "PayablesAndAccruedExpenses",
    "CurrentAccruedExpenses",
    "InterestPayable",
    "Payables",
    "DividendsPayable",
    "TotalTaxPayable",
    "IncomeTaxPayable",
    "AccountsPayable",
    "TotalAssets",
    "TotalNonCurrentAssets",
    "OtherNonCurrentAssets",
    "DefinedPensionBenefit",
    "NonCurrentPrepaidAssets",
    "NonCurrentDeferredAssets",
    "NonCurrentDeferredTaxesAssets",
    "NonCurrentNoteReceivables",
    "NonCurrentAccountsReceivable",
    "FinancialAssets",
    "InvestmentsAndAdvances",
    "OtherInvestments",
    "InvestmentinFinancialAssets",
    "HeldToMaturitySecurities",
    "AvailableForSaleSecurities",
    "TradingSecurities",
    "LongTermEquityInvestment",
    "InvestmentProperties",
    "GoodwillAndOtherIntangibleAssets",
    "OtherIntangibleAssets",
    "Goodwill",
    "NetPPE",
    "AccumulatedDepreciation",
    "GrossPPE",
    "Leases",
    "ConstructionInProgress",
    "OtherProperties",
    "MachineryFurnitureEquipment",
    "BuildingsAndImprovements",
    "LandAndImprovements",
    "Properties",
    "CurrentAssets",
    "OtherCurrentAssets",
    "HedgingAssetsCurrent",
    "AssetsHeldForSaleCurrent",
    "CurrentDeferredAssets",
    "CurrentDeferredTaxesAssets",
    "RestrictedCash",
    "PrepaidAssets",
    "Inventory",
    "FinishedGoods",
    "WorkInProcess",
    "RawMaterials",
    "Receivables",
    "OtherReceivables",
    "TaxesReceivable",
    "AccruedInterestReceivable",
    "NotesReceivable",
    "LoansReceivable",
    "AccountsReceivable",
    "GrossAccountsReceivable",
    "CashCashEquivalentsAndShortTermInvestments",
    "OtherShortTermInvestments",
    "CashAndCashEquivalents",
    "CashEquivalents",
    "CashFinancial",
];

const CASHFLOW_KEYS: &[&str] = &[
    "ForeignSales",
    "DomesticSales",
    "FreeCashFlow",
    "RepurchaseOfCapitalStock",
    "RepaymentOfDebt",
    "IssuanceOfDebt",
    "IssuanceOfCapitalStock",
    "CapitalExpenditure",
    "InterestPaidSupplementalData",
    "IncomeTaxPaidSupplementalData",
    "EndCashPosition",
    "BeginningCashPosition",
    "EffectOfExchangeRateChanges",
    "ChangesInCash",
    "CashFlowFromDiscontinuedOperation",
    "FinancingCashFlow",
    "CashFlowFromContinuingFinancingActivities",
    "NetOtherFinancingCharges",
    "InterestPaidCFF",
    "ProceedsFromStockOptionExercised",
    "CashDividendsPaid",
    "PreferredStockDividendPaid",
    "CommonStockDividendPaid",
    "NetPreferredStockIssuance",
    "PreferredStockPayments",
    "PreferredStockIssuance",
    "NetCommonStockIssuance",
    "CommonStockPayments",
    "CommonStockIssuance",
    "NetIssuancePaymentsOfDebt",
    "NetShortTermDebtIssuance",
    "ShortTermDebtPayments",
    "ShortTermDebtIssuance",
    "NetLongTermDebtIssuance",
    "LongTermDebtPayments",
    "LongTermDebtIssuance",
    "InvestingCashFlow",
    "CashFlowFromContinuingInvestingActivities",
    "NetOtherInvestingChanges",
    "NetInvestmentPurchaseAndSale",
    "SaleOfInvestment",
    "PurchaseOfInvestment",
    "NetBusinessPurchaseAndSale",
    "SaleOfBusiness",
    "PurchaseOfBusiness",
    "NetIntangiblesPurchaseAndSale",
    "SaleOfIntangibles",
    "PurchaseOfIntangibles",
    "NetPPEPurchaseAndSale",
    "SaleOfPPE",
    "PurchaseOfPPE",
    "CapitalExpenditureReported",
    "OperatingCashFlow",
    "CashFlowFromContinuingOperatingActivities",
    "TaxesRefundPaid",
    "InterestPaidCFO",
    "DividendPaidCFO",
    "ChangeInWorkingCapital",
    "ChangeInOtherWorkingCapital",
    "ChangeInOtherCurrentLiabilities",
    "ChangeInOtherCurrentAssets",
    "ChangeInPayablesAndAccruedExpense",
    "ChangeInAccruedExpense",
    "ChangeInPayable",
    "ChangeInAccountPayable",
    "ChangeInTaxPayable",
    "ChangeInIncomeTaxPayable",
    "ChangeInPrepaidAssets",
    "ChangeInInventory",
    "ChangeInReceivables",
    "ChangesInAccountReceivables",
    "OtherNonCashItems",
    "StockBasedCompensation",
    "UnrealizedGainLossOnInvestmentSecurities",
    "ProvisionandWriteOffofAssets",
    "AssetImpairmentCharge",
    "DeferredTax",
    "DeferredIncomeTax",
    "DepreciationAmortizationDepletion",
    "DepreciationAndAmortization",
    "AmortizationCashFlow",
    "AmortizationOfIntangibles",
    "Depreciation",
    "OperatingGainsLosses",
    "NetForeignCurrencyExchangeGainLoss",
    "GainLossOnSaleOfPPE",
    "GainLossOnSaleOfBusiness",
    "NetIncomeFromContinuingOperations",
];
