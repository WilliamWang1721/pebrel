$ErrorActionPreference = 'Stop'
$root = Join-Path $env:RUNNER_TEMP ('pebrel-issue195-' + [guid]::NewGuid().ToString('N'))
$share = 'PebrelIssue195' + [guid]::NewGuid().ToString('N')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
New-Item -ItemType Directory -Path $root | Out-Null
Set-Content -LiteralPath (Join-Path $root 'cwd-probe.cmd') -Encoding ascii -Value '@echo off', 'cd'
try {
    New-SmbShare -Name $share -Path $root -FullAccess $identity | Out-Null
    $unc = '\\localhost\' + $share
    if (-not (Test-Path -LiteralPath $unc)) { throw 'Disposable loopback UNC share is unavailable' }
    $env:PEBREL_ISSUE195_UNC = $unc
    $body = @'
$ErrorActionPreference = 'Continue'
$before = $PWD.ProviderPath
$cmd = & "$env:SystemRoot\System32\cmd.exe" /d /c cd 2>&1 | Out-String
$shim = & (Join-Path $env:PEBREL_ISSUE195_UNC 'cwd-probe.cmd') 2>&1 | Out-String
[ordered]@{ cwd = $before; cmd = $cmd; shim = $shim } | ConvertTo-Json -Compress
'@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
    $programs = @(
        (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'),
        (Get-Command pwsh.exe -ErrorAction Stop).Source
    )
    foreach ($program in $programs) {
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $program
        $info.Arguments = '-NoProfile -NonInteractive -EncodedCommand ' + $encoded
        $info.WorkingDirectory = $unc
        $info.UseShellExecute = $false
        $info.Environment.Remove('PSModulePath') | Out-Null
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $process = [Diagnostics.Process]::Start($info)
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        [ordered]@{ program = $program; exit = $process.ExitCode; stdout = $stdout; stderr = $stderr } | ConvertTo-Json -Compress
        if ($process.ExitCode -ne 0) { throw "PowerShell UNC reproduction failed: $stderr" }
        $result = $stdout.Trim() | ConvertFrom-Json
        if ($result.cwd.TrimEnd('\') -ne $unc.TrimEnd('\')) { throw 'PowerShell did not start in the requested UNC directory' }
        foreach ($output in @($result.cmd, $result.shim)) {
            $lines = @($output -split '\r?\n' | ForEach-Object { $_.Trim() })
            if ($lines -notcontains $env:SystemRoot) { throw 'CMD did not demonstrate the Windows-directory fallback' }
            if (-not $output.Contains('UNC')) { throw 'CMD UNC diagnostic is absent' }
        }
    }
} finally {
    Remove-SmbShare -Name $share -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
