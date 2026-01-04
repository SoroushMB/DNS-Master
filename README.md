# 🌐 DNS Speed Tester

A cross-platform Terminal User Interface (TUI) tool to benchmark DNS servers for **latency** and **download speed**.

![Rust](https://img.shields.io/badge/Rust-1.70+-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/License-MIT-green)

## ✨ Features

- 🚀 **Latency Testing** - Measures DNS resolution time
- 📥 **Download Speed Testing** - Tests CDN routing quality via each DNS
- 📊 **Sortable Results** - Sort by IP, latency, or download speed
- 🎨 **Beautiful TUI** - Built with [Ratatui](https://ratatui.rs/)
- 🔄 **Cross-Platform** - Works on Linux, macOS, and Windows

## 📦 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/dns-speed-tester.git
cd dns-speed-tester

# Build release binary
cargo build --release

# Run it
./target/release/DNS
```

### Prerequisites

- 🦀 Rust 1.70 or higher
- 📡 Internet connection for speed tests

## 🚀 Quick Start

```bash
# Run the application
cargo run --release
```

## 📖 Usage

### 1️⃣ Add DNS Servers

Type an IP address and press **Enter** to add it to the list:

```
8.8.8.8        # Google DNS
1.1.1.1        # Cloudflare DNS
9.9.9.9        # Quad9 DNS
208.67.222.222 # OpenDNS
```

### 2️⃣ Start Testing

Press **Tab** to begin the benchmark. The tool will:
1. 📍 Measure DNS resolution latency for each server
2. 📥 Test download speed through each DNS (1MB test file)

### 3️⃣ View Results

Results are displayed in a sortable table:

| DNS Server     | Latency    | Download (Mbps) | Status |
|----------------|------------|-----------------|--------|
| 1.1.1.1        | 12.34ms    | 89.45           | OK     |
| 8.8.8.8        | 15.67ms    | 76.32           | OK     |
| 9.9.9.9        | 23.45ms    | 65.21           | OK     |

## ⌨️ Keyboard Controls

| Key       | Action                          |
|-----------|---------------------------------|
| `Enter`   | ➕ Add DNS IP address           |
| `Backspace` | ❌ Remove character/last DNS   |
| `Tab`     | ▶️ Start testing                |
| `s`       | 🔄 Cycle sort column            |
| `d`       | ↕️ Toggle sort direction        |
| `r`       | 🔁 Run new test                 |
| `q`       | 🚪 Quit                         |

## 🛠️ How It Works

```mermaid
graph LR
    A[Enter DNS IPs] --> B[Start Test]
    B --> C[Measure Latency]
    C --> D[Test Download Speed]
    D --> E[Display Sorted Results]
```

1. **Latency Test**: Resolves `www.google.com` using each DNS server and measures response time
2. **Download Test**: Downloads a 1MB file from Cloudflare's speed test CDN after resolving through the target DNS

## 📁 Project Structure

```
src/
├── main.rs       # 🚀 Entry point & event loop
├── app.rs        # 📱 Application state management
├── ui.rs         # 🎨 TUI rendering
└── dns_utils.rs  # 🌐 DNS testing logic
```

## 🔧 Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | 🎨 Terminal UI framework |
| `crossterm` | ⌨️ Cross-platform terminal handling |
| `tokio` | ⚡ Async runtime |
| `hickory-resolver` | 🌐 DNS resolution |
| `reqwest` | 📥 HTTP client for speed tests |

## 🐛 Troubleshooting

### Permission Denied on External Drives

If you're running from an external drive without execute permissions:

```bash
CARGO_TARGET_DIR=/tmp/dns_target cargo run --release
```

### DNS Resolution Timeout

Increase timeout in `src/dns_utils.rs`:

```rust
opts.timeout = Duration::from_secs(10); // Default is 5
```

## 📄 License

MIT License - feel free to use and modify!

## 🤝 Contributing

Contributions welcome! Feel free to:
- 🐛 Report bugs
- 💡 Suggest features
- 🔧 Submit pull requests

---

Made with ❤️ and 🦀 Rust
