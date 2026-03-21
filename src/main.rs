use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};

use noahroboros::backtest::BacktestEngine;
use noahroboros::config::Config;
use noahroboros::market::download::{Downloader, Market};
use noahroboros::market::CsvLoader;
use noahroboros::strategy::baseline::BaselineStrategy;

#[derive(Parser)]
#[command(name = "noahroboros", about = "Autoresearch trading strategy optimizer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download historical OHLCV data from Binance
    Download {
        #[arg(short, long, value_delimiter = ',')]
        symbols: Option<Vec<String>>,

        #[arg(short, long)]
        interval: Option<String>,

        #[arg(long)]
        market: Option<String>,

        #[arg(short, long)]
        months: Option<u32>,

        #[arg(long)]
        start: Option<String>,

        #[arg(long)]
        end: Option<String>,

        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run backtest on historical data
    Backtest {
        #[arg(short, long)]
        data_dir: Option<String>,

        #[arg(short, long)]
        capital: Option<f64>,
    },
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env();
    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            symbols,
            interval,
            market,
            months,
            start,
            end,
            output,
        } => {
            let symbols = symbols.unwrap_or(cfg.download.symbols);
            let interval = interval.unwrap_or(cfg.download.interval);
            let market_str = market.unwrap_or(cfg.download.market);
            let market = Market::from_str(&market_str)?;
            let out_dir = output.unwrap_or(cfg.data_dir);

            let end_ms = match end {
                Some(ref d) => parse_date_ms(d).context("Invalid --end date")?,
                None => Utc::now().timestamp_millis(),
            };

            let start_ms = match start {
                Some(ref d) => parse_date_ms(d).context("Invalid --start date")?,
                None => {
                    let months = months.unwrap_or(cfg.download.months);
                    end_ms - (months as i64 * 30 * 24 * 3600 * 1000)
                }
            };

            let start_str = start.as_deref().unwrap_or("(auto)");
            let end_str = end.as_deref().unwrap_or("now");

            let downloader = Downloader::new(market);

            println!("Downloading {interval} {market} candles: {start_str} -> {end_str}");
            println!("Symbols: {}", symbols.join(", "));
            println!("Output:  {out_dir}/{interval}/");
            println!();

            for symbol in &symbols {
                print!("{symbol}... ");
                std::io::Write::flush(&mut std::io::stdout())?;
                match downloader.download(symbol, &interval, start_ms, end_ms, &out_dir) {
                    Ok(count) => println!("{count} candles"),
                    Err(e) => println!("FAILED: {e}"),
                }
            }

            println!(
                "\nDone. Run `cargo run --release -- backtest -d {out_dir}/{interval}` to test."
            );
        }

        Commands::Backtest { data_dir, capital } => {
            let data_dir = data_dir.unwrap_or(cfg.data_dir);
            let capital = capital.unwrap_or(cfg.initial_capital);

            let candles = CsvLoader::load_dir(&data_dir)?;
            if candles.is_empty() {
                anyhow::bail!(
                    "No candle data found in {data_dir}/. Run `cargo run -- download` first."
                );
            }

            eprintln!(
                "Backtesting {} candles, capital=${}, position={}%, fee={}bps, slip={}bps",
                candles.len(),
                capital,
                cfg.position_size_frac * 100.0,
                cfg.fee_bps,
                cfg.slippage_bps,
            );

            let mut strategy = BaselineStrategy::new();
            let mut engine =
                BacktestEngine::new(capital, cfg.position_size_frac, cfg.fee_bps, cfg.slippage_bps);
            let output = engine.run_with(&mut strategy, &candles);
            let scores = noahroboros::scoring::score(&output, capital, &cfg.scoring);

            println!("---");
            println!("score:              {:.3}", scores.score);
            println!("sharpe:             {:.3}", scores.sharpe);
            println!("total_return_pct:   {:.3}", scores.total_return_pct);
            println!("max_drawdown_pct:   {:.3}", scores.max_drawdown_pct);
            println!("num_trades:         {}", scores.num_trades);
            println!("win_rate_pct:       {:.3}", scores.win_rate_pct);
            println!("profit_factor:      {:.3}", scores.profit_factor);
            println!("annual_turnover:    {:.1}", scores.annual_turnover);
            println!("---");
        }
    }

    Ok(())
}

fn parse_date_ms(s: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("Expected date format YYYY-MM-DD, got: {s}"))?;
    let dt = date.and_hms_opt(0, 0, 0).unwrap();
    Ok(dt.and_utc().timestamp_millis())
}
