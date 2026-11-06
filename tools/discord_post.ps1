# Posts a text file (and optional attachments) to the #bot-update channel via webhook.
# Usage: powershell -File tools\discord_post.ps1 -File docs\outbox\update-002.txt [-Attach media\demo.mp4,docs\deck\deck.pdf]
# The webhook URL is read from the ESEC_WEBHOOK_UPDATES environment variable. Never put it in the repo.
# Safety: no @everyone/@here/role pings, 2000-char limit enforced, attachments capped at 9 MB each
# (bigger files are skipped with a note so the text still posts), sent text files move to docs/outbox/sent/.
param(
    [Parameter(Mandatory = $true)][string]$File,
    [string[]]$Attach = @()
)

$ErrorActionPreference = 'Stop'
$url = [Environment]::GetEnvironmentVariable('ESEC_WEBHOOK_UPDATES', 'User')
if (-not $url) { $url = $env:ESEC_WEBHOOK_UPDATES }
if (-not $url) { Write-Error 'ESEC_WEBHOOK_UPDATES is not set.'; exit 1 }
if ($url -notmatch '^https://(discord|discordapp)\.com/api/webhooks/') { Write-Error 'Webhook URL looks wrong.'; exit 1 }

$text = (Get-Content -Raw -Encoding UTF8 $File).Trim()
if ($text.Length -eq 0) { Write-Error 'File is empty.'; exit 1 }

$files = @()
$skipped = @()
$Attach = @($Attach | ForEach-Object { $_ -split ',' } | Where-Object { $_ })  # -File passes "a,b" as one string
foreach ($a in $Attach) {
    if (-not (Test-Path $a)) { $skipped += "$a (missing)"; continue }
    if ((Get-Item $a).Length -gt 9MB) { $skipped += "$(Split-Path $a -Leaf) (over 9 MB, share by link or drive)"; continue }
    $files += (Resolve-Path $a).Path
}
if ($skipped.Count -gt 0) { $text += "`nNot attached: " + ($skipped -join '; ') }
if ($text.Length -gt 1900) { Write-Error "Message is $($text.Length) chars; split it (limit 2000)."; exit 1 }

if ($files.Count -eq 0) {
    $body = @{ content = $text; allowed_mentions = @{ parse = @() } } | ConvertTo-Json -Depth 4
    Invoke-RestMethod -Method Post -Uri $url -ContentType 'application/json; charset=utf-8' `
        -Body ([System.Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}
else {
    # Discord allows up to 10 files per message; multipart via curl.exe (ships with Windows 10+).
    $payload = Join-Path $env:TEMP 'esec_payload.json'
    @{ content = $text; allowed_mentions = @{ parse = @() } } | ConvertTo-Json -Depth 4 |
        Set-Content -Encoding UTF8 $payload
    $args2 = @('-sS', '--fail', '-X', 'POST', '-F', "payload_json=<$payload;type=application/json")
    $i = 0
    foreach ($f in ($files | Select-Object -First 10)) { $args2 += @('-F', "files[$i]=@$f"); $i++ }
    $args2 += $url
    & curl.exe @args2 | Out-Null
    Remove-Item $payload -ErrorAction SilentlyContinue
    if ($LASTEXITCODE -ne 0) { Write-Error 'Upload failed.'; exit 1 }
}

$sent = Join-Path (Split-Path $File -Parent) 'sent'
New-Item -ItemType Directory -Force $sent | Out-Null
Move-Item $File (Join-Path $sent (Split-Path $File -Leaf))
Write-Output "Posted $File with $($files.Count) attachment(s)"
