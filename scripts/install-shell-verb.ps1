$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $projectRoot "target\release\wholocks-gui.exe"
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "WhoLocks release executable was not found at: $executable"
}

$legacyKeys = @(
    "HKCU:\Software\Classes\*\shell\WhoLocks",
    "HKCU:\Software\Classes\Directory\shell\WhoLocks",
    "HKCU:\Software\Classes\*\shell\WhoLocks.FindLocks",
    "HKCU:\Software\Classes\Directory\shell\WhoLocks.FindLocks",
    "HKCU:\Software\Classes\*\shell\OpenWithWhoLocks",
    "HKCU:\Software\Classes\Directory\shell\OpenWithWhoLocks"
)
$legacyKeys | ForEach-Object {
    Remove-Item -LiteralPath $_ -Recurse -Force -ErrorAction SilentlyContinue
}

$verbKey = "HKCU:\Software\Classes\AllFilesystemObjects\shell\WhoLocks"
$commandKey = Join-Path $verbKey "command"
New-Item -Path $commandKey -Force | Out-Null
Set-Item -LiteralPath $verbKey -Value "WhoLocks"
New-ItemProperty -LiteralPath $verbKey -Name "Icon" -Value $executable -PropertyType String -Force | Out-Null
Set-Item -LiteralPath $commandKey -Value ('"{0}" "%1"' -f $executable)

Write-Host "Installed WhoLocks for files and folders."
Write-Host "Command: `"$executable`" `"%1`""
