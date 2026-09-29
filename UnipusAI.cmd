@echo off
rem UnipusAI 菜单启动器
rem   UnipusAI.cmd                 -> 打开菜单
rem   UnipusAI.cmd progress --names -> 直接执行命令
setlocal EnableExtensions
chcp 65001 >nul
cd /d "%~dp0"
if exist "%~dp0tools\ffmpeg\bin\ffmpeg.exe" set "PATH=%~dp0tools\ffmpeg\bin;%PATH%"
if not defined HF_ENDPOINT set "HF_ENDPOINT=https://hf-mirror.com"
if not defined HF_HOME set "HF_HOME=%~dp0.whisper_cache"

if not "%~1"=="" goto run

:menu
cls
echo ================================================
echo    UnipusAI  U校园 AI 版 自动答题工具
echo ================================================
echo.
echo    0. 首次配置 / 换账号（粘贴 Cookie 或 cURL）
echo    1. 我的课程列表           （只读）
echo    2. 选择要刷的课程         （按序号）
echo    3. 查看课程结构与进度      （只读）
echo    4. 导出题目与音频转写      （只读）
echo    5. 题型自测               （不提交）
echo    6. 单组调试预览           （不提交）
echo    7. 音频识别测试           （试转写一段音频）
echo    8. 全量自动答题           （会真实提交作业）
echo    9. 退出
echo.
set "choice="
set /p "choice=请选择 0-9: "
if "%choice%"=="0" goto setup
if "%choice%"=="1" call :do courses & goto menu
if "%choice%"=="2" goto askcourse
if "%choice%"=="3" call :do progress --names & goto menu
if "%choice%"=="4" call :do dump-text --names & goto menu
if "%choice%"=="5" call :do test-types & goto menu
if "%choice%"=="6" goto askgroup
if "%choice%"=="7" goto askaudio
if "%choice%"=="8" goto askrun
if "%choice%"=="9" exit /b 0
goto menu

:setup
echo.
echo 把浏览器里「以 cURL 格式复制」的内容粘贴进来（粘贴完按 Ctrl+Z 再回车）：
echo.
"%~dp0UnipusAI.exe" init
echo.
pause
goto menu

:askcourse
echo.
echo 先看列表（如果还没列出过，可先回菜单选 1）
set "idx="
set /p "idx=输入课程序号（直接回车返回）: "
if "%idx%"=="" goto menu
call :do use %idx%
goto menu

:askgroup
echo.
set "gid="
set /p "gid=输入任务组 groupId（直接回车返回）: "
if "%gid%"=="" goto menu
call :do debug %gid%
goto menu

:askaudio
echo.
set "aurl="
set /p "aurl=音频地址（直接回车返回）: "
if "%aurl%"=="" goto menu
call :do transcribe %aurl%
goto menu

:askrun
echo.
echo 即将按 config.json 里的 learning_strategy 自动答题并提交。
echo 建议先做 5、6 两项确认效果。想只做必修就把 learning_strategy 改成
echo learn_all_compulsory_course；想全部做则改成 learn_all。
set "ok="
set /p "ok=确认开始请输入 YES: "
if /i not "%ok%"=="YES" goto menu
call :do run --interval 3000
goto menu

:do
echo.
echo ---- 正在执行: UnipusAI %* ----
echo.
"%~dp0UnipusAI.exe" %*
echo.
echo ---- 执行结束（退出码 %ERRORLEVEL%）----
pause
exit /b 0

:run
"%~dp0UnipusAI.exe" %*
exit /b %ERRORLEVEL%
