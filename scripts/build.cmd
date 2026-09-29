@echo off
rem 从源码编译（需要已安装 Rust 与 MSVC 生成工具）
setlocal
chcp 65001 >nul
cd /d "%~dp0.."

where cargo >nul 2>nul
if errorlevel 1 (
  echo [错误] 没找到 cargo。请先安装 Rust：https://rustup.rs
  echo        Windows 上还需要 Visual Studio 生成工具（C++ 生成工具）。
  pause
  exit /b 1
)

echo 正在编译（首次会下载依赖，耗时较长）...
cargo build --release
if errorlevel 1 (
  echo [错误] 编译失败。
  pause
  exit /b 1
)

copy /y "target\release\UnipusAI.exe" "UnipusAI.exe" >nul
echo.
echo 编译完成：UnipusAI.exe
pause
