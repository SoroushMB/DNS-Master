# 🌐 DNS & Mirror Master (Rust TUI)

A high-performance, cross-platform Terminal User Interface (TUI) tool for benchmarking DNS servers and Linux mirrors. Designed for power users who want speed, aesthetics, and reliability.

![Rust](https://img.shields.io/badge/Rust-1.88+-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/License-MIT-green)

---

## ✨ Features

### 🌐 DNS Speed Tester
- ⚡ **Latency Testing**: Measures DNS resolution time with high precision.
- 📥 **Download Benchmarking**: Checks CDN routing quality by testing real-world throughput.
- 🛡️ **Anti-Sanction Support**: Pre-loaded with Iranian anti-sanction DNS (Shecan, 403, Radar, etc.).
- 📊 **Real-time Graph**: Visualizes performance comparisons with a dynamic BarChart.

### 🪞 Mirror Master
- 🔍 **Auto-Distro Detection**: Automatically identifies your Linux distribution (Arch, Ubuntu, Debian, Kali, etc.).
- 📦 **Mirror Benchmarking**: Loads relevant package and tool mirrors (Docker, Android SDK) based on your OS.
- 🇮🇷 **Local Mirror Focus**: Specialized support for Iranian mirrors like Kubar, ArvanCloud, and Academic IDCs.

### 🎨 Premium TUI Experience
- 🎡 **Fluid Animations**: Smooth spinners and breathing pulsing effects for a modern feel.
- 🏎️ **Non-blocking Engine**: Benchmarks run in a background worker, ensuring the UI and animations stay responsive 100% of the time.
- ✨ **Visual feedback**: Vibrant emojis tailored to your OS distro and test status.

---

## 📦 Installation

### Prerequisites
- 🦀 **Rust 1.88** or higher.
- 📡 **Internet Connection** (required for benchmarks).

### Build from Source
```bash
# Clone and enter the repo
git clone https://github.com/SoroushMB/DNS-Master.git
cd DNS-Master

# Build for release
cargo build --release

# Run the app
./target/release/DNS
```

---

## 🚀 Usage Guide

### 1. 🌐 DNS Mode (Default)
Add DNS server IPs manually or load them via CLI. 
- **Type an IP** and press `Enter` to add it.
- **Press Tab** to start the test.
- **Watch the Graph**: See real-time download speed comparisons.
- **Apply Best**: Once finished, press `a` to apply the fastest DNS to your system (requires sudo/Admin).

### 2. 🪞 Mirror Master Mode
- **Toggle Mode**: Press `m` in the Input or Results screen to switch to Mirror mode.
- **Auto-Load**: The app automatically detects your distro and loads relevant mirrors from `examples/mirrors.csv`.
- **Benchmark**: Press `Tab` to test download speeds for each mirror. Useful for picking the fastest source for `apt`, `pacman`, or `docker`.

### 3. CLI Arguments
You can pre-load servers via command line:
```bash
# Comma-separated list
cargo run --release -- -d 8.8.8.8,1.1.1.1

# From CSV/JSON files
cargo run --release -- --csv examples/dns.csv --json custom_list.json
```

---

## ⌨️ Keyboard Controls

| Key           | Action                              |
|---------------|-------------------------------------|
| `m`           | 🔄 **Toggle Mode** (DNS ↔ Mirror)   |
| `Tab`         | ▶️ **Start Testing**                |
| `Enter`       | ➕ Add DNS IP (in DNS mode)         |
| `Backspace`   | ❌ Remove last character/server     |
| `s` / `d`     | 📊 Cycle Sort Column / Toggle Dir  |
| `a`           | 🛠️ **Apply Fastest DNS** to system   |
| `r`           | 🔁 Reset and start new test         |
| `q`           | 🚪 Quit                             |

> [!IMPORTANT]
> **System DNS Configuration (`a`)**:
> - **Linux**: Uses `nmcli` or `resolvectl` (requires `sudo`).
> - **macOS**: Uses `networksetup` (requires `sudo`).
> - **Windows**: Requires **Administrator Privileges**.

---

## 🔧 Technical Details

- **Concurrency**: Built with `tokio` channels (`mpsc`). The UI engine and the Network worker communicate asynchronously, preventing any lag or "ghosting" during heavy downloads.
- **Dynamic UI**: The `BarChart` uses a custom scaling algorithm to maintain visibility even when benchmarking 30+ servers simultaneously.
- **Timeout Logic**: A strict **7.5s hard limit** per server ensures the entire test suite stays within a predictable timeframe.

---

## 📁 Project Structure

```text
src/
├── main.rs         # Event loop & Worker orchestration
├── app.rs          # State management & Multi-mode logic
├── ui.rs           # Animated Ratatui components
├── dns_utils.rs    # Resolution & Download logic
├── mirror_utils.rs # Distro detection & Mirror testing
├── sys_dns.rs      # Cross-platform system configuration
└── file_loader.rs  # CSV/JSON parsing
```

---

## 📄 License
MIT License. Feel free to use, modify, and share!

Made with ❤️ and 🦀 Rust by **SoroushMB**
