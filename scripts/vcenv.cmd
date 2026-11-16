@echo off
rem Runs a command inside the x64 MSVC environment of Build Tools 2022.
rem Visual Studio 18 Community on this PC lacks the x64 libraries, so rustc must not pick it.
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
%*
