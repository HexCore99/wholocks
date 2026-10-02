$ErrorActionPreference = "Stop"

$whoLocksKeys = @(
    "HKCU:\Software\Classes\AllFilesystemObjects\shell\WhoLocks",
    "HKCU:\Software\Classes\*\shell\WhoLocks",
    "HKCU:\Software\Classes\Directory\shell\WhoLocks",
    "HKCU:\Software\Classes\*\shell\WhoLocks.FindLocks",
    "HKCU:\Software\Classes\Directory\shell\WhoLocks.FindLocks",
    "HKCU:\Software\Classes\*\shell\OpenWithWhoLocks",
    "HKCU:\Software\Classes\Directory\shell\OpenWithWhoLocks"
)

$whoLocksKeys | ForEach-Object {
    Remove-Item -LiteralPath $_ -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Removed the WhoLocks shell verb."
