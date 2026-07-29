use chrono::Utc;
use clap::Parser;
use cylindersense_core::payloads::TelemetryPayload;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────

/// CylinderSense device simulator — generates synthetic LPG telemetry.
#[derive(Parser, Debug)]
#[command(name = "cylindersense-simulator", version, about)]
struct Cli {
    /// Hardware identity string for the simulated device.
    #[arg(long, default_value = "sim-device-001")]
    device_id: String,

    /// Initial cylinder fill amount in kilograms.
    #[arg(long, default_value_t = 12.5)]
    fill_kg: f64,

    /// Cylinder tare (empty) weight in grams.
    #[arg(long, default_value_t = 5500)]
    tare_grams: i32,

    /// Interval between telemetry readings in seconds.
    #[arg(long, default_value_t = 5)]
    interval_secs: u64,

    /// URL of the ingest telemetry endpoint.
    #[arg(long, default_value = "http://localhost:3000/api/v1/telemetry")]
    endpoint: String,

    /// Standard deviation of Gaussian sensor noise in grams.
    #[arg(long, default_value_t = 15.0)]
    noise_stddev: f64,

    /// Gas consumption per cycle in grams.
    #[arg(long, default_value_t = 8.0)]
    drain_rate: f64,

    /// Probability (0.0–1.0) of a random weight spike per cycle.
    #[arg(long, default_value_t = 0.03)]
    spike_chance: f64,

    /// Remaining grams at which an automatic refill is triggered.
    #[arg(long, default_value_t = 200)]
    refill_threshold: i32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Simulation state
// ─────────────────────────────────────────────────────────────────────────────

struct SimState {
    remaining_grams: f64,
    fill_grams: f64,
    tare_grams: i32,
    cycle: u64,
}

impl SimState {
    fn new(fill_kg: f64, tare_grams: i32) -> Self {
        let fill_grams = fill_kg * 1000.0;
        Self {
            remaining_grams: fill_grams,
            fill_grams,
            tare_grams,
            cycle: 0,
        }
    }

    /// Drain gas, add noise/spikes, and return the raw load-cell reading.
    fn tick(
        &mut self,
        drain_rate: f64,
        noise_stddev: f64,
        spike_chance: f64,
        refill_threshold: i32,
    ) -> (i32, bool, bool) {
        self.cycle += 1;
        let mut rng = rand::thread_rng();

        // ── Drain ────────────────────────────────────────────────────
        self.remaining_grams = (self.remaining_grams - drain_rate).max(0.0);

        // ── Refill check ─────────────────────────────────────────────
        let refilled = if (self.remaining_grams as i32) <= refill_threshold {
            self.remaining_grams = self.fill_grams;
            true
        } else {
            false
        };

        // ── Noise ────────────────────────────────────────────────────
        let noise_dist = Normal::new(0.0, noise_stddev).expect("invalid noise stddev");
        let noise: f64 = noise_dist.sample(&mut rng);

        // ── Spike ────────────────────────────────────────────────────
        let spike_roll: f64 = rng.gen();
        let (spike, spiked) = if spike_roll < spike_chance {
            (rng.gen_range(2000.0..5000.0_f64), true)
        } else {
            (0.0, false)
        };

        // ── Raw reading ──────────────────────────────────────────────
        let raw = (self.tare_grams as f64) + self.remaining_grams + noise + spike;
        let raw_load_grams = raw.round() as i32;

        (raw_load_grams, spiked, refilled)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // ── Tracing ──────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    tracing::info!(
        device_id = %cli.device_id,
        fill_kg = %cli.fill_kg,
        tare_grams = %cli.tare_grams,
        interval = %cli.interval_secs,
        endpoint = %cli.endpoint,
        "starting CylinderSense simulator"
    );

    let client = reqwest::Client::new();
    let mut state = SimState::new(cli.fill_kg, cli.tare_grams);

    loop {
        let (raw_load_grams, spiked, refilled) = state.tick(
            cli.drain_rate,
            cli.noise_stddev,
            cli.spike_chance,
            cli.refill_threshold,
        );

        if refilled {
            tracing::info!(
                cycle = state.cycle,
                fill_grams = %state.fill_grams,
                "╔══ REFILL ══╗  cylinder refilled to {:.1} kg",
                state.fill_grams / 1000.0
            );
        }

        let payload = TelemetryPayload {
            device_id: cli.device_id.clone(),
            timestamp: Utc::now(),
            raw_load_grams,
        };

        // ── Log the reading ──────────────────────────────────────────
        let spike_tag = if spiked { " ⚡SPIKE" } else { "" };
        tracing::info!(
            cycle = state.cycle,
            remaining_g = %(state.remaining_grams as i32),
            raw = raw_load_grams,
            "→ POST {raw_load_grams}g{spike_tag}"
        );

        // ── Send to ingest ───────────────────────────────────────────
        match client.post(&cli.endpoint).json(&payload).send().await {
            Ok(resp) => {
                tracing::info!(status = %resp.status(), "  ← response");
            }
            Err(e) => {
                tracing::warn!(error = %e, "  ← connection failed (is ingest running?)");
            }
        }

        tokio::time::sleep(Duration::from_secs(cli.interval_secs)).await;
    }
}
