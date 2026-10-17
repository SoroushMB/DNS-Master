# 🌐 DNS-Master v2 (Rust TUI)

A fast, cross-platform **Terminal UI** that benchmarks DNS servers and — the new
headline feature — **empirically tests which DNS can actually open sanctioned
services from Iran**: Gemini, Google Antigravity, YouTube, Telegram, GitHub, X
and Spotify. No marketing claims, just live measurements on *your* network.

![Rust](https://img.shields.io/badge/Rust-1.88+-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/License-MIT-green)

---

## ✨ What's new in v2

- 📦 **Embedded DNS catalog** — 23 curated servers (Shecan, 403, Radar Game,
  Electro, Begzar, Pishgaman, Shatel, Vanilla + Google, Cloudflare, Quad9, …)
  baked into the binary. Works offline, extendable via CSV/JSON.
- 🎯 **Service Access Probes** — for every resolver the tool resolves each
  service through *that* DNS, connects with the proper SNI and classifies:
  ✅ unlocked · ✗ blocked (401/403/429/451/geo-page) · ⌛ timeout/filtered.
- ⚙️ **Parallel benchmark engine** — configurable lanes (default **2**, gentle
  on Iranian ISPs), epoch-based cancellation so `r`/`q` instantly frees
  bandwidth, watchdog deadlines so a wedged target can never hang the UI.
- 🏆 **Composite scoring** — 40% download speed, 30% unlocked services,
  20% latency, 10% stability — with per-column sorting.
- 💾 **Headless mode** — scriptable CLI run that prints a ranked table and exits.
- 💾 **Export** results to JSON/CSV (`e` key or `--export`).

---

## 📦 Installation

```bash
git clone https://github.com/SoroushMB/DNS-Master.git
cd DNS-Master
cargo build --release
./target/release/DNS
```

Prereqs: Rust 1.88+, internet.

---

## 🚀 Usage

### Interactive TUI

| Screen | Keys |
|--------|------|
| **Any** | **`q` / `Esc` / `Ctrl+C` quit** (works on every screen) |
| **Home** | `↑/↓` move · `space` include/exclude catalog entry · type IP + `Enter` add custom · `Backspace` remove custom · **`Tab` start test** |
| **Testing** | `r` cancel & go home |
| **Results** | `s` cycle sort column · `d` toggle direction · `↑/↓` scroll · **`a` apply best to system** · `e` export · `r` re-run · `h` home |

The Home screen lists the built-in catalog with each server's **IPv4 addresses
right next to its name** (primary + secondary), a ⚠️ marker for domestic-only
(private/carrier range) resolvers and a 🔒 marker when the provider offers DoH;
the details pane below shows the Persian name, category, DoH endpoint, known
services, notes and links. The Results table has a `# / Candidate / IP / Ping /
Mbps / <services> / OK / Score` layout that fits a standard 80-column terminal.

> [!IMPORTANT]
> Applying system DNS (`a`, or `--apply`): Linux uses `nmcli`/`resolvectl`,
> macOS `networksetup`, Windows `netsh` — sudo/Admin required.

### Headless runs

```bash
# Everything featured + your IPs, all 7 service probes
./target/release/DNS --non-interactive -d 158.220.x.x

# Only anti-sanction candidates, only Gemini/Antigravity probes
./target/release/DNS --non-interactive --category antisanction --verify gemini,antigravity

# Top results saved, best server applied automatically
./target/release/DNS --non-interactive --export results.json --apply
```

CLI flags: `-d/--dns`, `--json`, `--csv`, `--category`, `--verify`,
`--lanes N` (default 2), `--only-dns` (skip the built-in featured catalog and
test only your `-d` IPs), `--non-interactive`, `--export file(.json|.csv)`,
`--apply`, `--show-services`.

---

## 🇮🇷 Reading the results (from Iran)

- Straight Google `8.8.8.8` is a *baseline*: it will **not** open Gemini/YouTube
  — a low score here is expected and healthy.
- **Gemini** is DNS-fixable — providers like Shecan / 403 reverse-proxy it
  (here on a non-Iranian test network the probe correctly shows them behaving
  differently from the baseline).
- **YouTube / Telegram / X** are additionally blocked by *domestic filtering*;
  DNS alone may not open them on every ISP — the probe matrix tells you the
  truth per ISP/moment. Private-range resolvers (`10.x`, `⚠️` marker) only work
  while on an Iranian ISP connection.
- Probe outcomes are **per-ISP, per-moment**: export and compare runs from
  Irancell vs MCI vs home Wi-Fi (`results.json` / `results.csv`).

---

## 🔧 How it works (technical)

```text
src/
├── main.rs         # CLI, headless runner, TUI event loop
├── app.rs          # state machine: Home → Testing → Results
├── engine.rs       # bounded-parallel worker, epochs, scoring, sorting
├── dns_utils.rs    # latency + 1MB download benchmark via hickory-resolver
├── probe.rs        # per-service HTTPS access probes (SNI-pinned)
├── catalog.rs      # embedded catalog (assets/dns_catalog.json)
├── file_loader.rs  # user CSV/JSON server lists
├── sys_dns.rs      # cross-platform system DNS configuration
├── export.rs       # JSON/CSV export
└── ui.rs           # animated ratatui screens
```

- **Worker**: `tokio::spawn` per target, `Semaphore(lanes)` for concurrency.
  A newer test epoch **aborts all older tasks**, so cancelling is immediate.
- **Score**: `0.4·speed/max + 0.3·unlocked + 0.2·latency_norm + 0.1·ok`.
- **Harness deadline** per batch (`~20s/target`) — hung targets are marked
  `⌛ Aborted` and the flow always completes.

---

## 🧪 Tests

```bash
cargo test        # 9 unit tests: catalog parsing, scoring, sorting, dedup, export
cargo clippy -- -D warnings
```

---

## 📄 License

MIT License. Made with ❤️ and 🦀 by **SoroushMB**.
