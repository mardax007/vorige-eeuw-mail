@echo off
rem Runs a command with the Android SDK, NDK and a JDK set for this process only (phase 5).
rem Nothing is written to the Windows environment. The SDK lives in the default place that
rem Android Studio would use; the NDK is the newest one installed there.
set "ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk"
for /f "delims=" %%v in ('dir /b /ad /o:n "%ANDROID_HOME%\ndk" 2^>nul') do set "NDK_HOME=%ANDROID_HOME%\ndk\%%v"
rem Only a real JDK folder (jdk-*) with java.exe in it; "latest" is a link Gradle cannot use.
if not defined JAVA_HOME for /f "delims=" %%j in ('dir /b /ad /o:n "%ProgramFiles%\Java\jdk-*" 2^>nul') do if exist "%ProgramFiles%\Java\%%j\bin\java.exe" set "JAVA_HOME=%ProgramFiles%\Java\%%j"
set "PATH=%ANDROID_HOME%\platform-tools;%PATH%"
call "%~dp0vcenv.cmd" %*
