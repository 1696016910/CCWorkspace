# 安全重启 claude-workbench-rust
# 只按【精确进程名】结束应用，绝不使用 *Claude* 模糊匹配——
# 那会误杀用户终端里正在运行的 claude.exe 会话，强杀后终端残留
# 鼠标追踪模式，命令行刷出 [555;列;行M 乱码。
param(
    [string]$Exe = 'D:\Project\AI\CCWorkspace\rust\target\release\claude-workbench-rust.exe'
)
Get-Process -Name 'claude-workbench-rust' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
if (-not (Test-Path $Exe)) {
    Write-Error "找不到 $Exe，请先执行 cargo build --release"
    exit 1
}
Start-Process $Exe
Write-Output "已重启: $Exe"
