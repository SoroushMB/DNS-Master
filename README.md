# 🌐 DNS-Master v2 (Rust TUI + CLI)

A fast, cross-platform **Terminal UI** *and* **headless CLI** that benchmarks DNS
servers and — the headline feature — **empirically tests which DNS can actually
open sanctioned services from Iran**: Gemini, Google Antigravity, YouTube,
Telegram, GitHub, X and Spotify. No marketing claims, just live measurements on
*your* network.

![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![Catalog](https://img.shields.io/badge/catalog-41%20servers-brightgreen)
![License](https://img.shields.io/badge/License-MIT-green)

---

## Table of contents

- [What it is](#what-it-is)
- [⚠️ Read this first: sanctions vs. censorship](#️-read-this-first-sanctions-vs-censorship)
- [Features](#-features)
- [Requirements](#-requirements)
- [Installation](#-installation)
- [Quick start](#-quick-start)
- [The built-in catalog (41 servers)](#-the-built-in-catalog-41-servers)
- [Interactive TUI](#-interactive-tui)
- [Command-line reference](#️-command-line-reference)
- [Headless / scripting](#-headless--scripting)
- [Adding your own servers](#-adding-your-own-servers)
- [Service-access probes](#-service-access-probes)
- [Scoring](#-scoring)
- [Export formats](#-export-formats)
- [Applying system DNS](#-applying-system-dns)
- [Reading results from Iran](#-reading-results-from-iran)
- [Troubleshooting / FAQ](#-troubleshooting--faq)
- [Development](#-development)
- [License](#-license)

---

## What it is

DNS-Master does two independent things for every candidate resolver:

1. **Benchmarks the resolver itself** — DNS latency (a warm-up query) and a real
   **1 MB download** *through* that resolver, so you get a usable Mbps figure
   rather than a meaningless "ping".
2. **Verifies what it actually unlocks** — for each of seven services it resolves
   the service's domain *through the candidate DNS*, connects to the returned IP
   with the correct TLS SNI, and classifies the response: reachable, explicitly
   refused, or timed out. This is how you tell an honest anti-sanction DNS from a
   marketing page.

Everything is measured live on your own connection, then ranked with a composite
score. The whole catalog ships **inside the binary** (embedded JSON), so the tool
works fully offline and can be extended with your own CSV/JSON lists.

---

## ⚠️ Read this first: sanctions vs. censorship

These are two different problems and DNS only solves part of one of them. Being
clear about this saves you from buying or trusting the wrong tool:

| Problem | Example | Can a DNS change fix it? |
|---|---|---|
| **Sanctions / geo-blocking** — a foreign service refuses **Iranian IPs** | Google Gemini, some gaming launchers/stores, Apple services | **Sometimes.** Anti-sanction resolvers answer such domains through reachable routes/reverse proxies. This is what the 🇮🇷 category is for. |
| **State-level filtering (DPI)** — the state blocks a site at the network level | adult sites, and often YouTube/Telegram/X | **No.** The block is not in DNS; it is at the ISP/DPI layer. **No resolver reveals these** — you need a VPN / anti-filter that actually proxies traffic. |

A resolver ships in the catalog because it *resolves names*. It cannot route
around a network-level block. The probe matrix in the results screen shows you
the truth per ISP and per moment instead of promising magic.

---

## ✨ Features

- 📦 **Embedded catalog — 41 curated servers** (Iranian anti-sanction + public +
  privacy + gaming + family), baked into the binary. Works offline. Extendable
  via CSV/JSON.
- 🎯 **Service access probes** for Gemini, Antigravity, YouTube, Telegram,
  GitHub, X and Spotify — resolved *through the candidate* and classified
  `✓ unlocked · ✗ blocked · ⌛ timeout · ? error`.
- ⚙️ **Bounded-parallel engine** — configurable lanes (default **2**, gentle on
  Iranian ISPs), epoch-based cancellation so `r`/`q` frees bandwidth instantly,
  and per-target watchdogs so a wedged target can never hang the UI.
- 🏆 **Composite score** — 40 % download speed, 30 % unlocked services, 20 %
  latency, 10 % "resolver answered" — with per-column sorting.
- 🖥 **TUI + headless CLI** — the same engine behind an animated ratatui UI or a
  scriptable one-shot run that prints a ranked table and exits.
- 🔀 **`--all` / select-all** — benchmark the whole catalog, or just the featured
  defaults, or a single category.
- 💾 **Export** results to JSON/CSV (`e` key or `--export`).
- 🛡 **Safe by default** — applying DNS to the system is explicit (`a` / `--apply`).

---

## 📋 Requirements

- **Rust 1.85+** (edition 2024; tested on 1.99).
- A network connection.
- For applying system DNS: `sudo`/Administrator rights.
- Linux, macOS or Windows (cross-platform terminal via `crossterm`).

---

## 📦 Installation

### Build from source

```bash
git clone https://github.com/SoroushMB/DNS-Master.git
cd DNS-Master
cargo build --release
./target/release/DNS
```

### Install onto your `PATH`

```bash
cargo install --path .
# or copy the built binary
cp target/release/DNS /usr/local/bin/DNS     # macOS/Linux
```

### Cross-compile (optional)

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

---

## 🚀 Quick start

**Interactive (recommended first run):**

```bash
./target/release/DNS
```

Move with `↑/↓`, toggle servers with `space` (or press `a` to select **all**),
then press **`Tab`** to benchmark. After it finishes you are on the Results
screen: press `a` there to apply the best server to your system DNS, `e` to
export, `q`/`Esc` to quit.

**One-shot headless run (featured defaults + your own IPs, all 7 probes):**

```bash
./target/release/DNS --non-interactive -d 158.220.1.1
```

**Benchmark the entire catalog, only the anti-sanction group, Gemini probe only:**

```bash
./target/release/DNS --non-interactive --all --category antisanction --verify gemini
```

---

## 🗂 The built-in catalog (41 servers)

### Categories & markers

| Category | Meaning |
|---|---|
| 🇮🇷 **antisanction** | Iranian anti-sanction resolvers — the ones that can open sanctioned services |
| 🌍 **public** | Global public resolvers (Google, Cloudflare, Quad9, …) |
| 🎮 **gaming** | Gaming/ping-focused Iranian resolvers |
| 🛡️ **privacy** | No-log / privacy-first resolvers |
| 👨‍👩‍👧 **families** | Content-filtered profiles (may block adult sites) |

UI / table markers:

| Marker | Meaning |
|---|---|
| ⭐ | Part of the **featured** default set (what runs when you don't select anything) |
| ⚠️ | **Domestic-only** — private/carrier-range IP, reachable only from inside Iranian ISPs |
| ⛔ | **Content filter** — blocks adult sites, i.e. it *reduces* how many sites resolve |
| 🔒 | Provider also offers **DoH** (DNS-over-HTTPS) |

> [!NOTE]
> `⛔` resolvers are perfectly valid to *benchmark* — they are just labelled so you
> never pick one by mistake when your goal is to reach as many sites as possible.
> "Blocks malware only" profiles (Cloudflare `1.1.1.2`, CleanBrowsing security)
> are **not** marked `⛔`: they leave normal/adult browsing reachable.

### 🇮🇷 Anti-Sanction (رفع تحریم) — 12 servers

| Server | Primary | Secondary | Notes |
|---|---|---|---|
| Shecan ⭐ | `178.22.122.100` | `185.51.200.2` | Free tier (documented support for Google AI/Gemini, Steam, Apple, Docker). Official notice: the free tier is currently restricted and only usable with international access; the paid 'Shecan Pro' addresses are listed separately. |
| 403.online ⭐ | `10.202.10.202` | `10.202.10.102` | Private-range addresses: works only on Iranian ISP connections. DoH/DoT supported. |
| Radar Game ⭐ | `10.202.10.10` | `10.202.10.11` | Gaming-focused (National Computer Games Foundation). Private-range: Iranian ISPs only. |
| Electro ⭐ | `78.157.42.100` | `78.157.42.101` | Top pick for PSN/Xbox/launchers per community tests. |
| Electro (new) ⭐ | `78.157.108.21` | `78.157.108.22` | Newer published address set for Electro (stable ping for EA/FC 26). |
| Begzar ⭐ | `185.55.226.26` | `185.55.225.25` | General anti-sanction DNS, good complement for launchers. (Third official IP `185.55.224.24` also ships in the file.) |
| Pishgaman ⚠️ | `5.202.100.100` | `5.202.100.101` | Console/community favorite. Only reachable from within Iranian networks. |
| Shatel ⚠️ | `85.15.1.14` | `85.15.1.15` | Only recommended for Shatel ISP subscribers. |
| Zeus | `37.32.5.60` | `37.32.5.61` | Popular Iranian gaming/anti-sanction DNS; also advertised for Spotify, YouTube and OpenAI. Free. |
| Hostiran ⚠️ | `172.29.0.100` | `172.29.2.100` | Private-range addresses (172.16/12): reachable only from Iranian ISP connections. Vendor anti-sanction service. |
| Server.ir | `194.104.158.48` | `194.104.158.78` | Free anti-sanction DNS (Apple, GitHub, PSN and more). The official site issues addresses by signup; these are the widely published pair. |
| Shecan Pro | `178.22.122.101` | `185.51.200.1` | Paid Shecan tier (activated for your registered IP). The free tier is `178.22.122.100 / 185.51.200.2`. |

### 🌍 Public — 11 servers

| Server | Primary | Secondary | Notes |
|---|---|---|---|
| Google Public DNS ⭐ | `8.8.8.8` | `8.8.4.4` | Baseline for stability and downloads; does NOT bypass sanctions. |
| Cloudflare ⭐ | `1.1.1.1` | `1.0.0.1` | Fastest anycast resolver; does NOT bypass sanctions. |
| OpenDNS (Cisco) | `208.67.222.222` | `208.67.220.220` | Long-standing stable resolver. |
| Verisign | `64.6.64.6` | `64.6.65.6` | Operates .com/.net root infrastructure. |
| Neustar UltraDNS | `156.154.70.1` | `156.154.71.1` | Sometimes recommended for lower packet loss. |
| Hurricane Electric | `74.82.42.42` | — | Single anycast IP from HE in Houston. |
| Yandex | `77.88.8.8` | `77.88.8.1` | Geo-close alternative for some Iranian routes. Unfiltered (Basic) profile. |
| NTT | `129.250.35.250` | `129.250.35.251` | Suggested in Iranian gaming guides as a download/latency alternative. |
| DNS4EU (unfiltered) | `86.54.11.100` | `86.54.11.200` | EU-run public resolver (Whalebone) with no content filtering, under EU/GDPR rules. DoH: `https://unfiltered.joindns4.eu/dns-query`. |
| SafeDNS | `195.46.39.39` | `195.46.39.40` | Long-running public resolver with optional content filtering. |
| CZ.NIC ODVR | `193.17.47.1` | `185.43.135.1` | Run by CZ.NIC (the .cz registry): no logging, DNSSEC validation, Prague. DoH: `https://odvr.nic.cz/doh`. |

### 🎮 Gaming — 2 servers

| Server | Primary | Secondary | Notes |
|---|---|---|---|
| Shelter | `94.103.125.157` | `94.103.125.158` | Gaming anti-sanction DNS. Paid per-IP VIP plans; the shared public pair rotates, so treat these as best-effort. |
| Vanilla ⚠️ | `10.139.177.21` | `10.139.177.22` | Gaming DNS. Private-range: Iranian ISPs only. |

### 🛡️ Privacy — 8 servers

| Server | Primary | Secondary | Notes |
|---|---|---|---|
| Quad9 | `9.9.9.9` | `149.112.112.112` | Swiss no-log policy, blocks malicious domains. |
| AdGuard DNS | `94.140.14.14` | `94.140.15.15` | Blocks ads and trackers at DNS level. |
| Control D | `76.76.2.0` | `76.76.10.0` | Unfiltered `p0` anycast endpoints. |
| Comodo Secure | `8.26.56.26` | `8.20.247.20` | Security-filtered SECURE DNS by Comodo. |
| DNS.SB | `185.222.222.222` | `45.11.45.11` | No-log resolver, DNSSEC enabled. DoT: `dot.sb`. |
| DNS.WATCH | `84.200.69.80` | `84.200.70.40` | Uncensored no-log resolver, online since 2014, DNSSEC enabled. |
| UncensoredDNS | `91.239.100.100` | `89.233.43.71` | Danish no-log, uncensored resolver (anycast + unicast). **Best-effort**: on some networks TCP:53 connects but queries get no reply (network-level interception), so it may time out here while working on other ISPs. |
| NextDNS | `45.90.28.0` | `45.90.30.0` | Anycast resolver: applies your NextDNS profile when configured, otherwise plain unfiltered resolution. |

### 👨‍👩‍👧 Families (content-filtered) — 8 servers

| Server | Primary | Secondary | Notes |
|---|---|---|---|
| Cloudflare Families (malware) | `1.1.1.2` | `1.0.0.2` | Blocks malware domains **only**; adult content stays reachable (verified). |
| Cloudflare Families (adult) ⛔ | `1.1.1.3` | `1.0.0.3` | Blocks malware **and** adult content (verified: adult domains resolve to `0.0.0.0`). |
| Yandex Family ⛔ | `77.88.8.7` | `77.88.8.3` | Family profile: blocks adult content (verified: adult domains redirect to `safe2.yandex.ru`). |
| AdGuard Family ⛔ | `94.140.14.15` | `94.140.15.16` | Family-protection profile: ads, trackers and adult content. |
| CleanBrowsing (family) ⛔ | `185.228.168.168` | `185.228.169.168` | Family filter (verified: adult domains return NXDOMAIN). |
| CleanBrowsing (adult) ⛔ | `185.228.168.10` | `185.228.169.11` | Adult-content filter only (verified: adult domains return NXDOMAIN); also forces safe search. |
| OpenDNS FamilyShield ⛔ | `208.67.222.123` | `208.67.220.123` | OpenDNS profile that blocks adult content out of the box (verified). |
| CleanBrowsing (security) | `185.228.168.9` | `185.228.169.9` | Malware/phishing blocking only — adult content stays reachable (verified). |

> The `⛔` addresses were confirmed empirically with a live query against each:
> adult and malware test domains were resolved through the resolver and the
> answer checked for a sinkhole (`0.0.0.0`), an NXDOMAIN, or a redirect page.

---

## 🖥 Interactive TUI

Three screens: **Home → Testing → Results**.

### Home — catalog selection

```
┌ Catalog — built-in DNS servers ────────────────────────────────────────────┐
│ [✓] Shecan          178.22.122.100   185.51.200.2   G·Y·🔒                 │
│ [✓] 403.online       10.202.10.202   10.202.10.102  G·T·  ⚠️                │
│ [ ] Cloudflare … (adult)  1.1.1.3     1.0.0.3       ⛔                      │
└────────────────────────────────────────────────────────────────────────────┘
```

- Each row shows the **IPv4 addresses right next to the name** (primary + secondary).
- 🏷 The details pane below shows the Persian name, category, DoH endpoint,
  known services, notes and links for the entry under the cursor.

| Key | Action |
|---|---|
| `↑` / `↓` | Move the cursor through the catalog |
| `space` | Include / exclude the entry under the cursor |
| `a` | **Select every visible entry** (respects `--category`); press again to clear |
| `Enter` | Add the typed IP as a custom server |
| `Backspace` | Edit the input; on an empty input, remove the last custom IP |
| `Tab` | **Start benchmarking** the selected set |
| `q` / `Esc` / `Ctrl+C` | Quit |

### Testing — live progress

Shows a progress gauge, the active lanes with the current target and spinner,
the last result, the best-so-far with a pulsing border, and a live bar chart of
download speeds.

| Key | Action |
|---|---|
| `r` | Cancel and return Home |
| `q` / `Esc` / `Ctrl+C` | Quit |

### Results — ranked table

```
#   Candidate [role]  IP              Ping   Mbps  G A Y T H X S  OK  Score
1   Begzar [P]        185.55.226.26   420ms   2.6   ✓ ✓ ✓ ✓ ✗ ✓ ✓   6  0.88
```

Layout is `# / Candidate / IP / Ping / Mbps / <services> / OK / Score`, sized to
fit a standard **80-column** terminal. The `[role]` tag is `P` primary, `S`
secondary, `C` custom.

| Key | Action |
|---|---|
| `s` | Cycle sort column (Score → Unlocked → Ping → Speed → Candidate) |
| `d` | Toggle sort direction |
| `↑` `↓` `←` `→` | Scroll the table |
| `PageUp` / `PageDown` | Scroll a page |
| `a` | **Apply the best server to the system DNS** (needs sudo/Admin) |
| `e` | Export results to JSON/CSV |
| `r` | Re-run the same targets |
| `h` | Back Home |
| `q` / `Esc` / `Ctrl+C` | Quit |

---

## ⌨️ Command-line reference

```
DNS [OPTIONS]
```

| Flag | Description |
|---|---|
| `-d, --dns <IP,…>` | Add one or more custom DNS IPs (comma-separated). Always tested. |
| `--json <PATH>` | Load extra IPs from a JSON file `[{"ip":"…"}]`. |
| `--csv <PATH>` | Load extra IPs from a CSV file (header `ip` required). |
| `--category <CAT>` | Restrict the catalog to `antisanction`, `public`, `gaming`, `privacy` or `families`. |
| `--verify <SERVICES>` | Comma list of probes: `gemini,antigravity,youtube,telegram,github,x,spotify`. `all` (default) or `none`. Aliases: `yt,tg,gh,twitter`. |
| `--lanes <N>` | Parallel test lanes. Default **2** (conservative, best for Iranian ISPs). |
| `--non-interactive` | Headless: run, print a ranked table, exit. |
| `--export <PATH>` | Save results; extension picks the format (`.csv` or `.json`). |
| `--apply` | After testing, apply the best server as system DNS (needs sudo/Admin). |
| `--only-dns` | Test **only** the `-d` IPs (skip the featured catalog). |
| `--all` | Include **every** catalog entry, not just the featured defaults (combine with `--category`). |
| `-h, --help` | Print help. |
| `-V, --version` | Print version. |

### Examples

```bash
# Interactive, default featured set
DNS

# Interactive, only the families group
DNS --category families

# Headless: featured + two extra IPs, all probes, export JSON
DNS --non-interactive -d 1.1.1.1,8.8.8.8 --export results.json

# Headless: the entire catalog, sorted by score, then apply the winner
DNS --non-interactive --all --apply

# Headless: every anti-sanction server, only the Gemini probe, 4 lanes
DNS --non-interactive --all --category antisanction --verify gemini --lanes 4

# Headless: just my own two IPs, no catalog, no probes (pure speed test)
DNS --non-interactive --only-dns -d 9.9.9.9,1.1.1.1 --verify none

# Load a saved server list and export CSV
DNS --non-interactive --csv my_servers.csv --export out.csv
```

---

## 🤖 Headless / scripting

`--non-interactive` prints a ranked table to stdout and exits with a non-zero
code only if `--apply` fails:

```
🚀 Testing 16 targets at 8 lanes (7 service probes)...

#    Candidate                IP                      Ping       Mbps Access   Score
1    Begzar                   185.55.226.26          420ms        2.6 G✓A✓Y✓…  0.880

Legend: ✓ unlocked · ✗ blocked · ⌛ timeout · ? error
🏆 Best overall: Begzar (185.55.226.26) — score 0.880
```

It is safe to pipe/cron:

```bash
0 3 * * * /usr/local/bin/DNS --non-interactive --all --category antisanction \
          --verify none --export /var/log/dns/$(date +\%F).json
```

> If you select nothing headless (i.e. `--category` matches no **featured** entry
> and you pass neither `--all` nor `-d`), the run fails fast with
> *"No targets selected"*. Use `--all` or `-d`.

---

## ➕ Adding your own servers

Extra IPs are merged (and de-duplicated) with the catalog. Two file formats:

**JSON** — `servers.json`:

```json
[
  { "ip": "8.8.8.8" },
  { "ip": "1.1.1.1" }
]
```

**CSV** — `servers.csv` (header `ip`, case-insensitive column names):

```csv
ip
8.8.8.8
1.1.1.1
```

```bash
DNS --json servers.json --csv servers.csv
```

See [`examples/dns.json`](examples/dns.json) and [`examples/dns.csv`](examples/dns.csv).

---

## 🎯 Service-access probes

For each candidate the tool resolves the service domain **through that
resolver**, then connects to the resolved IP pinned with the right SNI:

| Glyph | Service | Probed host |
|---|---|---|
| `G` | Gemini | `gemini.google.com` |
| `A` | Antigravity | `antigravity.google` |
| `Y` | YouTube | `www.youtube.com` |
| `T` | Telegram | `web.telegram.org` |
| `H` | GitHub | `api.github.com` |
| `X` | X / Twitter | `x.com` |
| `S` | Spotify | `open.spotify.com` |

| Status | Symbol | Meaning |
|---|---|---|
| Unlocked | `✓` | Reachable (2xx/3xx, no geo-block signature in the body) |
| Blocked | `✗` | Explicitly refused: HTTP 401/403/429/451, or a geo-block interstitial |
| Timeout | `⌛` | DNS didn't resolve *or* the connection never established (domestic filtering / dead resolver) |
| Error | `?` | Ambiguous transport error (retried once before being reported) |

Because anti-sanction resolvers often answer through SNI-forwarding reverse
proxies, the probe is end-to-end: a `✓` means *the site really loaded through
that DNS*, not merely that a name resolved.

`--show-services` prints the exact probe targets and exits (hidden helper):

```bash
DNS --show-services
```

---

## 🏆 Scoring

Each result gets a 0.0–1.0 composite score:

```
score = 0.40 · speed_norm      (download Mbps ÷ best in the batch)
      + 0.30 · unlocked_norm   (unlocked services ÷ services probed)
      + 0.20 · latency_norm    (clamp((2500 − ms) ÷ 2500, 0, 1))
      + 0.10 · ok_norm         (1 if the resolver answered at all, else 0)
```

- When **no probes** are requested (`--verify none`), the unlocked term is a
  documented neutral **0.5** instead of penalising the resolver.
- Speed is normalised against the fastest resolver **in the same run**, so run
  comparable sets together.

---

## 💾 Export formats

`e` in the Results screen (or `--export <path>`) writes JSON or CSV by extension.

**JSON** — the raw result objects:

```json
[
  {
    "meta": { "cand": "Begzar", "ip": "185.55.226.26", "role": "Primary", "tags": [] },
    "dns": {
      "ip": "185.55.226.26",
      "latency": { "secs": 0, "nanos": 420000000 },
      "download_speed_mbps": 2.61,
      "error": null
    },
    "probes": [ ["gemini", "Unlocked"], ["github", "Blocked"] ]
  }
]
```

**CSV** columns:

```
candidate,ip,role,latency_ms,download_mbps,unlocked,checked,status,error
Begzar,185.55.226.26,primary,420.00,2.61,6,G✓A✓Y✓T✓H✗X✓S✓,Gemini:OK | GitHub:BLOCKED,
```

`checked` is the compact `G✓Y✗…` badge string; `status` is the human-readable
per-service summary.

---

## 🌍 Applying system DNS

Pressing `a` on Results (or `--apply`) sets the **best-scoring** server as the
system resolver:

| OS | Mechanism |
|---|---|
| Linux | `nmcli` / `resolvectl` |
| macOS | `networksetup` |
| Windows | `netsh` |

Requires `sudo`/Administrator. To revert, set your system DNS back to
**Automatic (DHCP)** or your ISP's resolver through the same OS settings.

> `--apply` refuses to apply a result whose resolver did not answer, so a dead
> candidate can never be written into your system config.

---

## 🇮🇷 Reading results from Iran

- Straight Google `8.8.8.8` is a **baseline**: it will *not* open Gemini/YouTube —
  a low score there is expected and healthy.
- **Gemini** is often DNS-fixable: providers like Shecan / 403 answer it through
  reachable routes. On a non-Iranian test network the probe shows them behaving
  differently from the baseline.
- **YouTube / Telegram / X** are additionally blocked by *domestic filtering* on
  many ISPs; DNS alone may not open them. The probe matrix tells you the truth
  per ISP and per moment.
- **Private-range** resolvers (`10.x`, `172.16/12`, `⚠️` marker) only work while
  you are actually on an Iranian ISP connection — they time out anywhere else.
- Results are **per-ISP, per-moment**: export and compare runs from Irancell vs
  MCI vs home Wi-Fi (`results.json` / `results.csv`).

### And the honest limit

If a site is blocked at the **national DPI/network layer** (adult content, and
often the social platforms), **no DNS change will open it** — including every
`⛔`/`🛡️` entry here. That requires a real proxy/VPN. DNS-Master will show you
this clearly (`✗` / `⌛` in the matrix) instead of pretending otherwise. To reach
*as many sites as possible*, prefer the 🇮🇷 **anti-sanction** resolvers for
sanctioned services and treat the `⛔` family profiles as benchmark targets, not
as ways to unlock content.

---

## 🧰 Troubleshooting / FAQ

**`No targets selected` in headless mode.**
You selected nothing to test. Add `--all` (whole catalog), `-d/--json/--csv`
(explicit IPs), or pick a category that has featured entries.

**Everything shows `⌛` / timeouts.**
Iranian private-range resolvers (`10.x`, `172.x`) only work from inside an
Iranian ISP. On a foreign connection they will all time out — that is expected,
not a bug.

**A resolver I know is alive shows `⌛`.**
Some networks silently intercept port 53 (TCP connects but queries get no reply).
UncensoredDNS is flagged this way; it may still work on your ISP.

**The download column shows `-`.**
The 1 MB HTTP download through that resolver did not complete (slow, filtered, or
the resolver doesn't forward well). Latency/probe results are still shown.

**Terminal looks garbled after quitting.**
The app restores the terminal on exit; if a hard kill interrupts it, run `reset`
(or `stty sane`) and re-open the terminal.

**Can I test IPv6 resolvers?**
The catalog ships IPv4 addresses; `-d` accepts any `IpAddr` the standard library
parses (IPv6 included) but the UI columns are sized for IPv4.

---

## 🛠 Development

```text
src/
├── main.rs         # CLI parsing, headless runner, TUI event loop, key handling
├── app.rs          # state machine: Home → Testing → Results, selection, sorting
├── engine.rs       # bounded-parallel worker, epochs, scoring, target building
├── dns_utils.rs    # latency + 1 MB download benchmark via hickory-resolver
├── probe.rs        # per-service HTTPS access probes (SNI-pinned)
├── catalog.rs      # embedded catalog model (assets/dns_catalog.json)
├── file_loader.rs  # user CSV/JSON server lists
├── sys_dns.rs      # cross-platform system DNS configuration
├── export.rs       # JSON/CSV export
└── ui.rs           # animated ratatui screens
```

Run the checks:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test           # unit tests: catalog, key handling, scoring, sorting, dedup, export
cargo build --release
```

The catalog lives in [`assets/dns_catalog.json`](assets/dns_catalog.json) and is
embedded with `include_str!`. Its top-level `version` is bumped whenever content
changes; edit it to add verified resolvers (keep addresses that actually answer,
and mark content filters with the `adult-filter` tag so the UI labels them).

---

## 📄 License

MIT License. Made with ❤️ and 🦀 by **SoroushMB**.
