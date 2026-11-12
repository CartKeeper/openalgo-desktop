//! Symbol metadata from FinanceDatabase (MIT).
//!
//! Pure parsing and exchange mapping for the FinanceDatabase equity and ETF
//! CSV files pinned at `FD_COMMIT`. See `docs/third-party/FinanceDatabase.md`.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

/// FinanceDatabase commit every download is pinned to.
pub const FD_COMMIT: &str = "1ac8809da480";
/// Date of the pinned commit's data.
pub const FD_DATA_DATE: &str = "2026-11-08";
/// Equity files (`database/equities/{code}.csv`) for the US exchanges.
pub const EQUITY_FILES: [&str; 8] = ["NYQ", "NMS", "NGM", "NCM", "ASE", "PCX", "BTS", "OQB"];
/// ETF files (`database/etfs/{code}.csv`) for the US exchanges.
pub const ETF_FILES: [&str; 6] = ["PCX", "BTS", "NGM", "NMS", "NYQ", "NCM"];
/// OTC pink-sheet equities file (14.5 MB), downloaded only when the user opts in.
pub const OTC_FILE: &str = "PNK";
/// `summary` is stored truncated to this many characters.
pub const SUMMARY_MAX_CHARS: usize = 2_000;

/// Map a FinanceDatabase exchange code to the app's exchange name.
/// Codes outside the US set return `None` and their rows are skipped.
pub fn map_exchange(code: &str) -> Option<&'static str> {
    match code.trim().to_ascii_uppercase().as_str() {
        "NYQ" => Some("NYSE"),
        "NMS" | "NGM" | "NCM" => Some("NASDAQ"),
        "ASE" => Some("AMEX"),
        "PCX" => Some("ARCA"),
        "BTS" => Some("BATS"),
        "OQB" | "PNK" => Some("OTC"),
        _ => None,
    }
}

/// One symbol's metadata, as stored in the DuckDB `symbol_metadata` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolMetadataRow {
    pub symbol: String,
    pub exchange: String,
    pub name: String,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub industry_group: Option<String>,
    pub country: Option<String>,
    pub market_cap: Option<f64>,
    pub size_bucket: Option<String>,
    pub isin: Option<String>,
    pub cusip: Option<String>,
    pub figi: Option<String>,
    pub website: Option<String>,
    pub summary: Option<String>,
    /// `"equity"` or `"etf"`.
    pub asset_type: String,
    pub delisted: bool,
}

#[derive(Clone, Copy)]
enum AssetKind {
    Equity,
    Etf,
}

impl AssetKind {
    fn label(self) -> &'static str {
        match self {
            AssetKind::Equity => "equity",
            AssetKind::Etf => "etf",
        }
    }
}

/// Parse a FinanceDatabase equities file.
///
/// Header: `symbol,name,summary,currency,sector,industry_group,industry,exchange,mic,market,
/// country,state,city,zipcode,website,market_cap,isin,cusip,figi,composite_figi,shareclass_figi,delisted`.
/// In this dataset `market_cap` is a size bucket ("Large Cap"), so it is kept in
/// `size_bucket` and `market_cap` stays `None`.
pub fn parse_equities(csv_text: &str) -> Result<Vec<SymbolMetadataRow>> {
    parse(csv_text, AssetKind::Equity)
}

/// Parse a FinanceDatabase ETFs file.
///
/// Header: `symbol,name,currency,summary,category_group,category,family,exchange,mic,isin,delisted`;
/// `category_group` → `sector`, `category` → `industry`, `family` → `industry_group`.
pub fn parse_etfs(csv_text: &str) -> Result<Vec<SymbolMetadataRow>> {
    parse(csv_text, AssetKind::Etf)
}

/// Column positions looked up by header name, so a reordered file still parses.
struct Header(csv::StringRecord);

impl Header {
    fn index(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|h| h.trim().eq_ignore_ascii_case(name))
    }

    fn required(&self, name: &str) -> Result<usize> {
        self.index(name).ok_or_else(|| {
            AppError::Validation(format!("FinanceDatabase CSV is missing the '{name}' column"))
        })
    }
}

fn parse(csv_text: &str, kind: AssetKind) -> Result<Vec<SymbolMetadataRow>> {
    // A UTF-8 byte-order mark would otherwise become part of the first header.
    let text = csv_text.strip_prefix('\u{feff}').unwrap_or(csv_text);

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let header = Header(reader.headers().map_err(csv_error)?.clone());

    let i_symbol = header.required("symbol")?;
    let i_name = header.required("name")?;
    let i_exchange = header.required("exchange")?;
    let (i_sector, i_industry, i_industry_group) = match kind {
        AssetKind::Equity => (
            header.index("sector"),
            header.index("industry"),
            header.index("industry_group"),
        ),
        AssetKind::Etf => (
            header.index("category_group"),
            header.index("category"),
            header.index("family"),
        ),
    };
    let i_summary = header.index("summary");
    let i_country = header.index("country");
    let i_market_cap = header.index("market_cap");
    let i_isin = header.index("isin");
    let i_cusip = header.index("cusip");
    let i_figi = header.index("figi");
    let i_website = header.index("website");
    let i_delisted = header.index("delisted");

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(csv_error)?;
        let field = |i: Option<usize>| -> Option<String> {
            i.and_then(|i| record.get(i))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };

        let Some(symbol) = field(Some(i_symbol)) else {
            continue;
        };
        let Some(exchange) = field(Some(i_exchange)).as_deref().and_then(map_exchange) else {
            continue;
        };
        let name = field(Some(i_name)).unwrap_or_else(|| symbol.clone());
        let delisted = field(i_delisted)
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        rows.push(SymbolMetadataRow {
            symbol,
            exchange: exchange.to_string(),
            name,
            sector: field(i_sector),
            industry: field(i_industry),
            industry_group: field(i_industry_group),
            country: field(i_country),
            market_cap: None,
            size_bucket: field(i_market_cap),
            isin: field(i_isin),
            cusip: field(i_cusip),
            figi: field(i_figi),
            website: field(i_website),
            summary: field(i_summary).map(|s| truncate_chars(s, SUMMARY_MAX_CHARS)),
            asset_type: kind.label().to_string(),
            delisted,
        });
    }

    Ok(rows)
}

/// Keep at most `max` characters (not bytes), so multi-byte text stays valid.
fn truncate_chars(s: String, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((byte_index, _)) => s[..byte_index].to_string(),
        None => s,
    }
}

fn csv_error(e: csv::Error) -> AppError {
    AppError::Validation(format!("FinanceDatabase CSV parse error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EQUITIES_HEADER: &str = "symbol,name,summary,currency,sector,industry_group,industry,exchange,mic,market,country,state,city,zipcode,website,market_cap,isin,cusip,figi,composite_figi,shareclass_figi,delisted";

    /// Three rows: a quoted summary with a comma and an embedded newline, an
    /// empty name, and a row on an exchange the app does not map (LSE).
    fn equities_fixture() -> String {
        format!(
            "{EQUITIES_HEADER}\n\
             A,\"Agilent Technologies, Inc.\",\"Agilent provides solutions, worldwide.\nSecond line of summary.\",USD,Health Care,\"Pharmaceuticals, Biotechnology & Life Sciences\",Diagnostics & Research,NYQ,XNYS,us_market,United States,CA,Santa Clara,95051,https://www.agilent.com,Large Cap,US00846U1016,00846U101,BBG000C2V541,BBG000C2V3D6,BBG001SCTQY4,False\n\
             NONAME,,,USD,Technology,Software & Services,Software - Application,NMS,XNAS,us_market,United States,,,,,Small Cap,,,,,,True\n\
             LSEONLY,Some UK Plc,A UK company.,GBP,Industrials,Capital Goods,Machinery,LSE,XLON,gb_market,United Kingdom,,,,,Mid Cap,,,,,,False\n"
        )
    }

    const ETFS_HEADER: &str = "symbol,name,currency,summary,category_group,category,family,exchange,mic,isin,delisted";

    fn etfs_fixture() -> String {
        format!(
            "{ETFS_HEADER}\n\
             AAA,AAF First Priority CLO Bond ETF,USD,\"The investment seeks capital preservation and income.\n The fund is actively managed.\",Fixed Income,Corporate Bonds,Exchange Traded Concepts,PCX,ARCX,US46144X6105,False\n"
        )
    }

    /// Build one equities line from the 22 header positions, quoting as CSV requires.
    fn equity_line(set: &[(usize, &str)]) -> String {
        let mut fields = vec![String::new(); 22];
        for (i, v) in set {
            fields[*i] = (*v).to_string();
        }
        fields
            .iter()
            .map(|f| {
                if f.contains([',', '"', '\n']) {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    #[test]
    fn map_exchange_covers_the_us_codes_and_rejects_others() {
        assert_eq!(map_exchange("NYQ"), Some("NYSE"));
        assert_eq!(map_exchange("NMS"), Some("NASDAQ"));
        assert_eq!(map_exchange("NGM"), Some("NASDAQ"));
        assert_eq!(map_exchange("NCM"), Some("NASDAQ"));
        assert_eq!(map_exchange("ASE"), Some("AMEX"));
        assert_eq!(map_exchange("PCX"), Some("ARCA"));
        assert_eq!(map_exchange("BTS"), Some("BATS"));
        assert_eq!(map_exchange("OQB"), Some("OTC"));
        assert_eq!(map_exchange("PNK"), Some("OTC"));
        assert_eq!(map_exchange("LSE"), None);
        assert_eq!(map_exchange(""), None);
    }

    #[test]
    fn pinned_source_constants() {
        assert_eq!(FD_COMMIT, "1ac8809da480");
        assert_eq!(FD_DATA_DATE, "2026-11-08");
        assert_eq!(EQUITY_FILES, ["NYQ", "NMS", "NGM", "NCM", "ASE", "PCX", "BTS", "OQB"]);
        assert_eq!(ETF_FILES, ["PCX", "BTS", "NGM", "NMS", "NYQ", "NCM"]);
        assert_eq!(OTC_FILE, "PNK");
    }

    #[test]
    fn parse_equities_keeps_mapped_rows_and_quoted_summary_intact() {
        let rows = parse_equities(&equities_fixture()).expect("fixture parses");
        assert_eq!(rows.len(), 2, "the LSE row is skipped");

        let a = &rows[0];
        assert_eq!(a.symbol, "A");
        assert_eq!(a.exchange, "NYSE");
        assert_eq!(a.name, "Agilent Technologies, Inc.");
        assert_eq!(
            a.summary.as_deref(),
            Some("Agilent provides solutions, worldwide.\nSecond line of summary.")
        );
        assert_eq!(a.sector.as_deref(), Some("Health Care"));
        assert_eq!(
            a.industry_group.as_deref(),
            Some("Pharmaceuticals, Biotechnology & Life Sciences")
        );
        assert_eq!(a.industry.as_deref(), Some("Diagnostics & Research"));
        assert_eq!(a.country.as_deref(), Some("United States"));
        assert_eq!(a.size_bucket.as_deref(), Some("Large Cap"));
        assert_eq!(a.market_cap, None, "the dataset's market_cap column is a size bucket");
        assert_eq!(a.isin.as_deref(), Some("US00846U1016"));
        assert_eq!(a.cusip.as_deref(), Some("00846U101"));
        assert_eq!(a.figi.as_deref(), Some("BBG000C2V541"));
        assert_eq!(a.website.as_deref(), Some("https://www.agilent.com"));
        assert_eq!(a.asset_type, "equity");
        assert!(!a.delisted);
    }

    #[test]
    fn parse_equities_fills_an_empty_name_with_the_symbol_and_reads_delisted() {
        let rows = parse_equities(&equities_fixture()).expect("fixture parses");
        let noname = &rows[1];
        assert_eq!(noname.symbol, "NONAME");
        assert_eq!(noname.name, "NONAME");
        assert_eq!(noname.exchange, "NASDAQ");
        assert!(noname.delisted, "'True' parses as delisted");
        assert_eq!(noname.summary, None, "empty fields become None");
        assert_eq!(noname.isin, None);
        assert_eq!(noname.sector.as_deref(), Some("Technology"));
    }

    #[test]
    fn parse_equities_accepts_a_utf8_bom_before_the_header() {
        let text = format!("\u{feff}{}", equities_fixture());
        let rows = parse_equities(&text).expect("BOM-prefixed fixture parses");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "A");
    }

    #[test]
    fn parse_equities_reads_delisted_case_insensitively() {
        let text = format!(
            "{EQUITIES_HEADER}\n{}\n{}\n{}\n",
            equity_line(&[(0, "X1"), (1, "X One"), (7, "NYQ"), (21, "TRUE")]),
            equity_line(&[(0, "X2"), (1, "X Two"), (7, "NYQ"), (21, "false")]),
            equity_line(&[(0, "X3"), (1, "X Three"), (7, "NYQ")]),
        );
        let rows = parse_equities(&text).expect("parses");
        assert_eq!(rows.len(), 3);
        assert!(rows[0].delisted);
        assert!(!rows[1].delisted);
        assert!(!rows[2].delisted, "an empty delisted field means listed");
    }

    #[test]
    fn parse_equities_truncates_summary_to_2000_characters() {
        let long_summary = "é".repeat(2_500);
        let text = format!(
            "{EQUITIES_HEADER}\n{}\n",
            equity_line(&[(0, "LONG"), (1, "Long Summary Co"), (2, &long_summary), (7, "NYQ")])
        );
        let rows = parse_equities(&text).expect("parses");
        let summary = rows[0].summary.as_deref().expect("summary kept");
        assert_eq!(summary.chars().count(), 2_000);
    }

    #[test]
    fn parse_equities_rejects_a_file_without_a_symbol_column() {
        let err = parse_equities("name,exchange\nFoo,NYQ\n").expect_err("missing symbol column");
        assert!(err.to_string().contains("symbol"), "error names the missing column: {err}");
    }

    #[test]
    fn parse_etfs_maps_category_fields_onto_sector_industry_and_group() {
        let rows = parse_etfs(&etfs_fixture()).expect("fixture parses");
        assert_eq!(rows.len(), 1);
        let aaa = &rows[0];
        assert_eq!(aaa.symbol, "AAA");
        assert_eq!(aaa.name, "AAF First Priority CLO Bond ETF");
        assert_eq!(aaa.exchange, "ARCA");
        assert_eq!(aaa.sector.as_deref(), Some("Fixed Income"), "category_group -> sector");
        assert_eq!(aaa.industry.as_deref(), Some("Corporate Bonds"), "category -> industry");
        assert_eq!(
            aaa.industry_group.as_deref(),
            Some("Exchange Traded Concepts"),
            "family -> industry_group"
        );
        assert_eq!(aaa.isin.as_deref(), Some("US46144X6105"));
        assert_eq!(
            aaa.summary.as_deref(),
            Some("The investment seeks capital preservation and income.\n The fund is actively managed.")
        );
        assert_eq!(aaa.country, None);
        assert_eq!(aaa.cusip, None);
        assert_eq!(aaa.size_bucket, None);
        assert_eq!(aaa.asset_type, "etf");
        assert!(!aaa.delisted);
    }
}
