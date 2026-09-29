@echo off
chcp 65001 >nul
cd /d "%~dp0"
set "PATH=%~dp0tools\ffmpeg\bin;%PATH%"
if not defined HF_ENDPOINT set "HF_ENDPOINT=https://hf-mirror.com"
if not defined HF_HOME set "HF_HOME=%~dp0.whisper_cache"

echo ==========================================================
echo  UnipusAI 首次配置
echo ==========================================================
echo.
echo  接下来需要你把「带 Cookie 的请求」粘贴进来：
echo    1) 浏览器登录 U校园（ucontent.unipus.cn）并进入任意课程
echo    2) 按 F12 -^> 网络(Network) -^> 点 Fetch/XHR 过滤
echo    3) 刷新页面，右键任意 ucontent.unipus.cn 的请求
echo       -^> 复制 -^> 以 cURL 格式复制（Copy as cURL）
echo    4) 回到本窗口，粘贴（可多行），然后按 Ctrl+Z 再回车结束输入
echo.
pause
"%~dp0UnipusAI.exe" init
echo.
echo 如果上面列出了你的课程，用下面这条命令选择课程（把序号换成你要刷的那门）：
echo     UnipusAI.exe use 1
echo.
pause
