@echo off
rem Builds snes_play.exe from the C that "codegen --all" wrote in this folder.
rem Needs Visual Studio Build Tools and the SDL2 development package for Visual C++ (SDL2-devel-x.y.z-VC.zip):
rem set SDL2_DIR to its SDL2-x.y.z folder (the one with include and lib), then run this script from this folder.
if "%SDL2_DIR%"=="" (
  echo error: set SDL2_DIR to the folder of the SDL2 development package ^(with include and lib^)
  exit /b 2
)
where cl >nul 2>&1 || call :find_vs
where cl >nul 2>&1 || (
  echo error: Visual Studio Build Tools not found; run this from an "x64 Native Tools Command Prompt"
  exit /b 2
)
if not exist obj mkdir obj
cl /nologo /MP /O2 /TC /std:c11 /DSNES_RT_TICK /I. /I"%SDL2_DIR%\include" rom_*.c snes_entry.c play\snes_play.c /Fe:snes_play.exe /Fo:obj\ /link /LIBPATH:"%SDL2_DIR%\lib\x64" SDL2.lib || exit /b 1
copy /y "%SDL2_DIR%\lib\x64\SDL2.dll" . >nul
echo built snes_play.exe
exit /b 0

:find_vs
set "VSW=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if exist "%VSW%" (
  for /f "usebackq delims=" %%i in (`"%VSW%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do call "%%i\VC\Auxiliary\Build\vcvars64.bat" >nul
)
exit /b 0
