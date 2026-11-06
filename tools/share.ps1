# Start the Casefile demo and print a link a groupmate can open.
#   tools\share.ps1            # same Wi-Fi / Tailscale: prints LAN and Tailscale URLs
#   tools\share.ps1 -Tunnel    # also opens a public https URL through localhost.run (needs ssh)
# The demo is replay mode on synthetic data. Public mode keeps audit entries in a temp folder,
# capped at 200, and resets them on every start. Close this window to stop sharing.
param([int]$Port = 8080, [switch]$Tunnel)

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
cargo build --release -p casefile
if ($LASTEXITCODE -ne 0) { exit 1 }

$env:CASEFILE_ROOT = $root
$srv = Start-Process -PassThru -NoNewWindow -FilePath "$root\target\release\casefile.exe" -ArgumentList @("serve", "--public", "--port", $Port)
Start-Sleep -Seconds 2
Write-Host "Local:     http://localhost:$Port/"
$ips = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notmatch '^(127|169\.254)\.' }
foreach ($ip in $ips) { Write-Host "Network:   http://$($ip.IPAddress):$Port/   ($($ip.InterfaceAlias))" }
Write-Host "(Windows Firewall may ask to allow casefile.exe on private networks: allow it.)"
try {
    if ($Tunnel) {
        Write-Host "Opening public tunnel; the https URL is printed below..."
        ssh -o ServerAliveInterval=30 -o StrictHostKeyChecking=accept-new -R "80:localhost:$Port" nokey@localhost.run
    } else {
        Wait-Process -Id $srv.Id
    }
} finally {
    if (-not $srv.HasExited) { Stop-Process -Id $srv.Id }
}
