use anyhow::{Result, Context, anyhow};
use std::net::IpAddr;
use std::process::Command;

/// Set the system DNS to the specified IP address.
/// Supports Linux, Windows, and macOS.
pub fn set_system_dns(dns_ip: IpAddr) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        set_linux_dns(dns_ip)
    }
    #[cfg(target_os = "windows")]
    {
        set_windows_dns(dns_ip)
    }
    #[cfg(target_os = "macos")]
    {
        set_macos_dns(dns_ip)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(anyhow!("System DNS configuration is not supported on this operating system."))
    }
}



#[cfg(target_os = "windows")]
fn set_windows_dns(dns_ip: IpAddr) -> Result<()> {
    // 1. Find the primary interface name (active and connected)
    // We use powershell to get the interface name because it's more reliable than netsh parsing
    let output = Command::new("powershell")
        .args(["-Command", "Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | Select-Object -ExpandProperty Name"])
        .output()
        .context("Failed to run powershell to get network adapters")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let interface = stdout.lines().next().context("No active network adapters found")?.trim();

    // 2. Set DNS via netsh (Requires Administrator)
    let status = Command::new("netsh")
        .args(["interface", "ip", "set", "dns", &format!("name=\"{}\"", interface), "source=static", &format!("addr={}", dns_ip)])
        .status()
        .context("Failed to run netsh. Ensure you are running as Administrator.")?;

    if !status.success() {
        return Err(anyhow!("netsh command failed. Ensure the terminal is running as Administrator."));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_macos_dns(dns_ip: IpAddr) -> Result<()> {
    // 1. Get the primary network service
    let output = Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .context("Failed to list network services")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Usually the first one that isn't a header is the active one, 
    // but we can try to find one with an IP
    let mut service = "Wi-Fi"; // Default 
    for s in stdout.lines().skip(1) {
        if s.starts_with('*') { continue; } // Skip disabled
        let info = Command::new("networksetup")
            .args(["-getinfo", s])
            .output();
        if let Ok(i) = info {
            if String::from_utf8_lossy(&i.stdout).contains("IP address:") {
                service = s;
                break;
            }
        }
    }

    // 2. Set DNS (Requires sudo)
    let status = Command::new("sudo")
        .args(["networksetup", "-setdnsservers", service, &dns_ip.to_string()])
        .status()
        .context("Failed to run sudo networksetup")?;

    if !status.success() {
        return Err(anyhow!("sudo networksetup failed. Ensure you have sudo privileges."));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn set_linux_dns(dns_ip: IpAddr) -> Result<()> {
    // Try nmcli first (NetworkManager)
    if let Ok(_) = Command::new("nmcli").arg("--version").output() {
        match set_via_nmcli(dns_ip) {
            Ok(_) => return Ok(()),
            Err(e) => {
                // If it fails (maybe not using NM), fall back to resolvectl
                eprintln!("nmcli failed: {}. Trying resolvectl...", e);
            }
        }
    }
    
    // Try resolvectl (systemd-resolved)
    if let Ok(_) = Command::new("resolvectl").arg("--version").output() {
        return set_via_resolvectl(dns_ip);
    }

    Err(anyhow!("Could not find nmcli or resolvectl to configure DNS. Please ensure you have NetworkManager or systemd-resolved installed."))
}

#[cfg(target_os = "linux")]
fn set_via_nmcli(dns_ip: IpAddr) -> Result<()> {
    // 1. Get active connection Name
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
        .output()
        .context("Failed to run nmcli show active")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Find the first non-loopback connection (usually ethernet or wifi)
    let conn_name = stdout.lines()
        .filter(|line| !line.contains("loopback"))
        .next()
        .context("No active network connections found via nmcli")?
        .split(':')
        .next()
        .context("Failed to parse connection name")?;

    // 2. Set DNS (Using sudo - user must have sudo access or run as root)
    // Note: In TUI, sudo might prompt for password.
    let status = Command::new("sudo")
        .args(["nmcli", "connection", "modify", conn_name, "ipv4.dns", &dns_ip.to_string()])
        .status()
        .context("Failed to run sudo nmcli modify")?;
    
    if !status.success() {
        return Err(anyhow!("sudo nmcli modify failed. Ensure you have sudo privileges."));
    }

    // 3. Apply changes (Reload the connection)
    let status = Command::new("sudo")
        .args(["nmcli", "connection", "up", conn_name])
        .status()
        .context("Failed to run sudo nmcli up")?;
    
    if !status.success() {
        return Err(anyhow!("sudo nmcli up failed to apply changes."));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn set_via_resolvectl(dns_ip: IpAddr) -> Result<()> {
    // Find default interface
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .context("Failed to run ip route show default")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let dev = stdout.split_whitespace().collect::<Vec<_>>();
    let dev_index = dev.iter().position(|&s| s == "dev").context("Could not find 'dev' in default route")?;
    let interface = dev.get(dev_index + 1).context("Could not find interface name after 'dev'")?;

    // Set DNS via resolvectl
    let status = Command::new("sudo")
        .args(["resolvectl", "dns", interface, &dns_ip.to_string()])
        .status()
        .context("Failed to run sudo resolvectl dns")?;

    if !status.success() {
        return Err(anyhow!("sudo resolvectl failed. Ensure you have sudo privileges."));
    }

    // Flush caches
    let _ = Command::new("sudo")
        .args(["resolvectl", "flush-caches"])
        .status();

    Ok(())
}
