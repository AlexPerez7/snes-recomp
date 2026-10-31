# Arma out\release\snes-port-<version>-win64.zip: el programa, la documentación, la licencia y los ficheros de
# desarrollo de SDL2 (licencia zlib). El motor no lleva perfiles de juegos (cada juego tiene su repositorio y su
# paquete) ni ROMs, partidas guardadas o C generado: si algo así acaba en la carpeta, el script falla y no crea el zip.
#
#   powershell -File scripts\release.ps1 [-Sdl2 <carpeta con include y lib>] [-Extra <carpeta a añadir>] [-NoTcc]
#
# -Extra existe para comprobar la guarda (se copia tal cual a la carpeta y se revisa como todo lo demás).
# El paquete lleva TinyCC (LGPL) en toolchain\tcc para que quien no tiene compilador no tenga que descargar nada
# (snes-port lo usa si la máquina no tiene uno propio). Se descarga con la URL y el SHA-256 de src\download.rs y se
# guarda en out\tcc; con -NoTcc el paquete no lo lleva y snes-port ofrece descargarlo la primera vez.
param(
    [string]$Sdl2 = "",
    [string]$Extra = "",
    [switch]$NoBuild,
    [switch]$NoTcc
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $NoBuild) {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build fallo" }
}
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$name = "snes-port-$version-win64"
$stage = Join-Path $root "out\release\$name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item target\release\snes-port.exe, target\release\snes-recomp.exe $stage
Copy-Item README.md, LICENSE $stage

if ($Sdl2 -eq "") {
    $found = Get-ChildItem out\sdl2 -Directory -ErrorAction SilentlyContinue | Where-Object { Test-Path "$($_.FullName)\include\SDL.h" } | Select-Object -First 1
    if ($found) { $Sdl2 = $found.FullName }
}
if ($Sdl2 -ne "") {
    $dst = Join-Path $stage "sdl2"
    New-Item -ItemType Directory -Force "$dst\lib" | Out-Null
    Copy-Item "$Sdl2\include" $dst -Recurse
    Copy-Item "$Sdl2\lib\x64" "$dst\lib" -Recurse
    if (Test-Path "$Sdl2\COPYING.txt") { Copy-Item "$Sdl2\COPYING.txt" "$dst\COPYING.txt" }
} else {
    Write-Warning "sin SDL2: el zip no lleva la carpeta sdl2 (hay que dar SDL2_DIR al usarlo)"
}
if ($Extra -ne "") { Copy-Item $Extra $stage -Recurse }

if (-not $NoTcc) {
    # misma fuente y mismo hash que usa snes-port para descargarlo: se leen de src\download.rs para no repetirlos
    $src = Get-Content src\download.rs -Raw
    $url = [regex]::Match($src, 'url: "([^"]+tcc[^"]+\.zip)"').Groups[1].Value
    $sha = [regex]::Match($src, 'sha256: "([0-9a-f]{64})"').Groups[1].Value
    if ($url -eq "" -or $sha -eq "") { throw "no encuentro la URL o el SHA-256 de TinyCC en src\download.rs" }
    $zipTcc = Join-Path $root "out\tcc\$([IO.Path]::GetFileName($url))"
    New-Item -ItemType Directory -Force (Split-Path $zipTcc) | Out-Null
    if (-not (Test-Path $zipTcc) -or (Get-FileHash $zipTcc -Algorithm SHA256).Hash.ToLower() -ne $sha) {
        curl.exe --fail --silent --show-error --location --max-time 300 --output $zipTcc $url
        if ($LASTEXITCODE -ne 0) { throw "no se pudo descargar TinyCC de $url" }
    }
    $got = (Get-FileHash $zipTcc -Algorithm SHA256).Hash.ToLower()
    if ($got -ne $sha) { Remove-Item -Force $zipTcc; throw "TinyCC no tiene el SHA-256 esperado (esperado $sha, obtenido $got): descartado" }
    $tool = Join-Path $stage "toolchain"
    New-Item -ItemType Directory -Force $tool | Out-Null
    Expand-Archive -Path $zipTcc -DestinationPath $tool -Force
    if (-not (Test-Path "$tool\tcc\tcc.exe")) { throw "TinyCC se descomprimio pero falta toolchain\tcc\tcc.exe" }
    if (-not (Test-Path "$tool\tcc\doc") -and -not (Get-ChildItem "$tool\tcc" -Filter "COPYING*" -Recurse)) { Write-Warning "el zip de TinyCC no trae su licencia a la vista; revisa toolchain\tcc" }
    @"
TinyCC $([IO.Path]::GetFileNameWithoutExtension($url)) (LGPL 2.1)

Este compilador de C viaja con el paquete solo para construir el juego en tu maquina cuando no tienes uno propio
(Visual Studio Build Tools, gcc o clang): snes-port lo usa en ese caso y nunca lo ejecuta si ya tienes uno.
Es software libre bajo la licencia LGPL; su licencia y su documentacion estan en la carpeta tcc.
Codigo fuente: https://download.savannah.gnu.org/releases/tinycc/ y https://repo.or.cz/tinycc.git
Descargado de $url (SHA-256 $sha).
"@ | Set-Content -Encoding utf8 "$tool\AVISO-TINYCC.txt"
}

# la guarda: nada de ROMs, partidas guardadas ni C generado
$bad = Get-ChildItem $stage -Recurse -File | Where-Object {
    $_.Extension -in ".sfc", ".smc", ".srm", ".sst", ".fig", ".swc" -or
    $_.Name -like "rom_*.c" -or $_.Name -in "snes_entry.c", "snes_funcs.h", "snes_headless.c", "snes_play.c"
}
if ($bad) {
    $bad | ForEach-Object { Write-Error "prohibido en el paquete: $($_.FullName)" -ErrorAction Continue }
    Remove-Item -Recurse -Force $stage
    throw "el paquete llevaria ficheros de ROM o C generado: no se crea el zip"
}

$zip = Join-Path $root "out\release\$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "creado $zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MiB)"
