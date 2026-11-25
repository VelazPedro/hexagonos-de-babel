# Builds the single-file game and leaves a copy at the project root.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#
# The result needs no other files: content is built in and the C runtime is
# linked statically. Paths of this machine are rewritten so that they do not
# end up inside the distributed program.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

$flags = @("-Ctarget-feature=+crt-static")
$remaps = [ordered]@{
    "$env:USERPROFILE\.cargo\registry\src" = "deps"
    "$env:USERPROFILE\.cargo"              = "cargo"
    "$env:USERPROFILE\.rustup"             = "rustup"
    $root                                  = "game"
    $env:USERPROFILE                       = "home"
}
foreach ($from in $remaps.Keys) {
    $flags += "--remap-path-prefix=$from=$($remaps[$from])"
}
# The encoded form is the only one that survives spaces in a path.
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1f

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$built = Join-Path $root "target\release\hexagonos_de_babel.exe"
$copy = Join-Path $root "hexagonos_de_babel.exe"
if (Get-Process -Name hexagonos_de_babel -ErrorAction SilentlyContinue) {
    Write-Error "Cerrá el juego antes de reemplazar $copy"
}
Copy-Item -LiteralPath $built -Destination $copy -Force

# Refuse to hand out a program that mentions this user or this folder.
$bytes = [System.IO.File]::ReadAllBytes($copy)
$text = [System.Text.Encoding]::GetEncoding(28591).GetString($bytes)
$private = @($env:USERNAME, (Split-Path -Leaf $root), "\Users\")
$leaks = $private | Where-Object { $_ -and $text.IndexOf($_, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 }
if ($leaks) {
    Write-Error "El ejecutable todavía contiene datos de esta máquina: $($leaks -join ', ')"
}
"Listo: $copy ($([math]::Round((Get-Item -LiteralPath $copy).Length / 1MB, 1)) MB)"
